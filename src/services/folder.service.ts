import type { Types } from 'mongoose';

import { FileModel } from '../models/file.model';
import type { FolderHydrated } from '../models/folder.model';
import { FolderModel } from '../models/folder.model';
import { forbidden, notFound } from '../utils/httpError';

import { deleteObject } from './r2.service';

interface ListFoldersParams {
  parentId: string | null;
  viewerId?: string;
}

/**
 * Danh sách folder nhìn thấy được: public luôn thấy, private chỉ chủ sở hữu thấy.
 */
export const listFolders = (params: ListFoldersParams): Promise<FolderHydrated[]> => {
  const { parentId, viewerId } = params;

  const visibility = viewerId
    ? { $or: [{ isPublic: true }, { ownerId: viewerId }] }
    : { isPublic: true };

  return FolderModel.find({ parentId, ...visibility }).sort({ name: 1 });
};

export const listAllUserFolders = (ownerId: string): Promise<FolderHydrated[]> => {
  return FolderModel.find({ ownerId }).sort({ name: 1 });
};

export const createFolder = (params: {
  name: string;
  parentId: string | null;
  isPublic: boolean;
  ownerId: string;
}): Promise<FolderHydrated> => {
  return FolderModel.create(params);
};

const getOwnedFolder = async (folderId: string, ownerId: string): Promise<FolderHydrated> => {
  const folder = await FolderModel.findById(folderId);
  if (!folder) {
    throw notFound('Không tìm thấy thư mục');
  }
  if (folder.ownerId !== ownerId) {
    throw forbidden();
  }
  return folder;
};

export const updateFolder = async (
  folderId: string,
  ownerId: string,
  updates: { name?: string; parentId?: string | null; isPublic?: boolean },
): Promise<FolderHydrated> => {
  const folder = await getOwnedFolder(folderId, ownerId);
  Object.assign(folder, updates);
  await folder.save();
  return folder;
};

/**
 * Xoá đệ quy: tất cả folder con, file trong folder con, và file trực tiếp
 * trong folder này, kèm xoá object thật trên R2. Không dùng transaction vì
 * R2 nằm ngoài MongoDB nên không thể atomic tuyệt đối; ưu tiên xoá R2 trước,
 * xoá DB sau để tránh rác record trỏ tới object đã mất.
 */
export const deleteFolderRecursive = async (folderId: string, ownerId: string): Promise<void> => {
  await getOwnedFolder(folderId, ownerId);

  const childFolders = await FolderModel.find({ parentId: folderId, ownerId }, '_id');
  await Promise.all(childFolders.map((child) => deleteFolderRecursive(String(child._id), ownerId)));

  const files = await FileModel.find({ folderId, ownerId });
  await Promise.all(
    files.map(async (file) => {
      await deleteObject(file.key);
      await file.deleteOne();
    }),
  );

  await FolderModel.deleteOne({ _id: folderId, ownerId });
};

export interface BreadcrumbEntry {
  id: string;
  name: string;
}

/**
 * Build breadcrumb từ root đến folder hiện tại (dùng cho view EJS).
 * Chỉ kiểm tra visibility của folder hiện tại (folder sâu nhất) — giả định
 * cấu trúc nhất quán, tức folder cha luôn cùng ownerId với folder con.
 */
export const getBreadcrumb = async (
  folderId: string | null,
  viewerId?: string,
): Promise<BreadcrumbEntry[]> => {
  if (!folderId) {
    return [];
  }

  let current = await FolderModel.findById(folderId);
  if (!current) {
    throw notFound('Không tìm thấy thư mục');
  }
  if (!current.isPublic && current.ownerId !== viewerId) {
    throw forbidden();
  }

  const chain: BreadcrumbEntry[] = [];
  while (current) {
    chain.unshift({ id: String(current._id), name: current.name });
    current = current.parentId ? await FolderModel.findById(current.parentId) : null;
  }
  return chain;
};

export const assertFolderVisible = async (
  folderId: Types.ObjectId | string,
  viewerId?: string,
): Promise<void> => {
  const folder = await FolderModel.findById(folderId);
  if (!folder) {
    throw notFound('Không tìm thấy thư mục');
  }
  if (!folder.isPublic && folder.ownerId !== viewerId) {
    throw forbidden();
  }
};

/**
 * Kiểm tra quyền sở hữu chặt: dùng khi ghi dữ liệu vào folder (upload file mới).
 * Khác assertFolderVisible ở chỗ không cho phép chỉ vì folder là public —
 * folder public của người khác vẫn không được phép upload vào.
 */
export const assertFolderOwnership = async (
  folderId: Types.ObjectId | string,
  ownerId: string,
): Promise<void> => {
  const folder = await FolderModel.findById(folderId);
  if (!folder) {
    throw notFound('Không tìm thấy thư mục');
  }
  if (folder.ownerId !== ownerId) {
    throw forbidden();
  }
};
