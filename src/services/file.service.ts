import { LRUCache } from 'lru-cache';

import type { FileHydrated } from '../models/file.model';
import { FileModel } from '../models/file.model';
import { badRequest, forbidden, notFound } from '../utils/httpError';

const fileCache = new LRUCache<string, FileHydrated>({
  max: 500,
  ttl: 1000 * 60 * 5, // 5 minutes
});

import { assertFolderOwnership } from './folder.service';
import {
  buildObjectKey,
  createDownloadUrl,
  createUploadUrl,
  deleteObject,
  getObjectMeta,
} from './r2.service';

interface ListFilesParams {
  folderId: string | null;
  viewerId?: string;
}

export const listFiles = (params: ListFilesParams): Promise<FileHydrated[]> => {
  const { folderId, viewerId } = params;

  const visibility = viewerId
    ? { $or: [{ isPublic: true }, { ownerId: viewerId }] }
    : { isPublic: true };

  return FileModel.find({ folderId, status: 'completed', ...visibility }).sort({ createdAt: -1 });
};

export const requestUploadUrl = async (params: {
  name: string;
  mimeType: string;
  size: number;
  folderId: string | null;
  isPublic: boolean;
  ownerId: string;
}): Promise<{ fileId: string; uploadUrl: string }> => {
  const { name, mimeType, size, folderId, isPublic, ownerId } = params;

  if (folderId) {
    // Folder cha phải tồn tại và thuộc về chính user này mới cho phép upload vào
    await assertFolderOwnership(folderId, ownerId);
  }

  const key = buildObjectKey(ownerId, name);

  const file = await FileModel.create({
    name,
    key,
    size,
    mimeType,
    folderId,
    ownerId,
    isPublic,
    status: 'pending',
  });

  const uploadUrl = await createUploadUrl(key, mimeType);

  return { fileId: String(file._id), uploadUrl };
};

const getOwnedFile = async (fileId: string, ownerId: string): Promise<FileHydrated> => {
  const file = await FileModel.findById(fileId);
  if (!file) {
    throw notFound('Không tìm thấy file');
  }
  if (file.ownerId !== ownerId) {
    throw forbidden();
  }
  return file;
};

/**
 * Xác nhận upload lên R2 đã hoàn tất: kiểm tra object thật sự tồn tại trên R2
 * trước khi đánh dấu completed, tránh trường hợp client gọi complete gian dối
 * hoặc do lỗi mạng khiến upload thật ra chưa xong.
 */
export const completeUpload = async (fileId: string, ownerId: string): Promise<FileHydrated> => {
  const file = await getOwnedFile(fileId, ownerId);

  if (file.status === 'completed') {
    return file;
  }

  const meta = await getObjectMeta(file.key);
  if (!meta.exists) {
    file.status = 'failed';
    await file.save();
    throw badRequest('Không tìm thấy file trên R2, upload có thể đã thất bại');
  }

  file.status = 'completed';
  if (meta.size !== undefined) {
    file.size = meta.size;
  }
  await file.save();

  fileCache.set(fileId, file);
  return file;
};

export const getDownloadUrl = async (
  fileId: string,
  viewerId?: string,
  inline: boolean = false,
): Promise<string> => {
  let file: FileHydrated | null | undefined = fileCache.get(fileId);
  if (!file) {
    file = await FileModel.findById(fileId);
    if (file) {
      fileCache.set(fileId, file);
    }
  }

  if (!file || file.status !== 'completed') {
    throw notFound('Không tìm thấy file');
  }
  if (!file.isPublic && file.ownerId !== viewerId) {
    throw forbidden();
  }

  // Cập nhật số lượt mở/tải file bất đồng bộ (không await) để giảm thời gian phản hồi
  const updateQuery = inline ? { $inc: { views: 1 } } : { $inc: { downloads: 1 } };
  FileModel.updateOne({ _id: fileId }, updateQuery).exec().catch(console.error);

  if (file.externalUrl) {
    return file.externalUrl;
  }
  return createDownloadUrl(file.key, file.name, inline);
};

export const deleteFile = async (fileId: string, ownerId: string): Promise<void> => {
  const file = await getOwnedFile(fileId, ownerId);
  if (!file.externalUrl) {
    await deleteObject(file.key);
  }
  await file.deleteOne();
  fileCache.delete(fileId);
};

export const createLinkedFile = async (params: {
  name: string;
  url: string;
  mimeType: string;
  folderId: string | null;
  ownerId: string;
}): Promise<FileHydrated> => {
  const { name, url, mimeType, folderId, ownerId } = params;

  if (folderId) {
    await assertFolderOwnership(folderId, ownerId);
  }

  // Use a random key to satisfy uniqueness, though not used for R2.
  const key = `linked-${ownerId}-${Date.now()}-${Math.random().toString(36).substring(7)}`;

  const file = await FileModel.create({
    name,
    key,
    size: 0,
    mimeType,
    externalUrl: url,
    folderId,
    ownerId,
    isPublic: false,
    status: 'completed',
  });

  return file;
};

export const updateFile = async (
  fileId: string,
  ownerId: string,
  updates: { name?: string; isPublic?: boolean; folderId?: string | null; url?: string; mimeType?: string },
): Promise<FileHydrated> => {
  const file = await getOwnedFile(fileId, ownerId);

  if (updates.folderId !== undefined) {
    if (updates.folderId !== null) {
      await assertFolderOwnership(updates.folderId, ownerId);
    }
  }
  
  const applyUpdates: Record<string, unknown> = { ...updates };
  if (updates.url !== undefined) {
    applyUpdates.externalUrl = updates.url;
    delete applyUpdates.url;
  }
  
  Object.assign(file, applyUpdates);
  await file.save();
  fileCache.set(fileId, file);
  return file;
};
