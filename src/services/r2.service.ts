import {
  DeleteObjectCommand,
  GetObjectCommand,
  HeadObjectCommand,
  PutObjectCommand,
} from '@aws-sdk/client-s3';
import { getSignedUrl } from '@aws-sdk/s3-request-presigner';
import { v4 as uuidv4 } from 'uuid';

import { env } from '../config/env';
import { r2Client } from '../config/r2';

/**
 * Sinh object key duy nhất trên R2, phân theo ownerId để dễ soát/dọn dẹp,
 * giữ nguyên tên gốc (đã encode) để dễ debug khi xem trực tiếp trên dashboard R2.
 */
export const buildObjectKey = (ownerId: string, originalName: string): string => {
  const safeName = originalName.replace(/[^a-zA-Z0-9._-]/g, '_');
  return `${ownerId}/${uuidv4()}-${safeName}`;
};

export const createUploadUrl = async (key: string, mimeType: string): Promise<string> => {
  const command = new PutObjectCommand({
    Bucket: env.R2_BUCKET_NAME,
    Key: key,
    ContentType: mimeType,
  });
  return getSignedUrl(r2Client, command, { expiresIn: env.R2_UPLOAD_URL_EXPIRES_IN });
};

export const createDownloadUrl = async (key: string, downloadName?: string): Promise<string> => {
  const command = new GetObjectCommand({
    Bucket: env.R2_BUCKET_NAME,
    Key: key,
    ...(downloadName && {
      ResponseContentDisposition: `attachment; filename="${encodeURIComponent(downloadName)}"`,
    }),
  });
  return getSignedUrl(r2Client, command, { expiresIn: env.R2_DOWNLOAD_URL_EXPIRES_IN });
};

/**
 * Kiểm tra object đã thực sự tồn tại trên R2 chưa (dùng ở bước "complete" upload),
 * đồng thời trả về size thật để đối chiếu với size client khai báo ban đầu.
 */
export const getObjectMeta = async (key: string): Promise<{ exists: boolean; size?: number }> => {
  try {
    const result = await r2Client.send(
      new HeadObjectCommand({ Bucket: env.R2_BUCKET_NAME, Key: key }),
    );
    return { exists: true, size: result.ContentLength };
  } catch {
    return { exists: false };
  }
};

export const deleteObject = async (key: string): Promise<void> => {
  await r2Client.send(new DeleteObjectCommand({ Bucket: env.R2_BUCKET_NAME, Key: key }));
};
