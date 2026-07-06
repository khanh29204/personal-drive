/* eslint-disable no-console */
import { ListObjectsV2Command, DeleteObjectCommand } from '@aws-sdk/client-s3';

import { connectDatabase } from '../config/db';
import { env } from '../config/env';
import { r2Client } from '../config/r2';
import { FileModel } from '../models/file.model';

const BATCH_SIZE = 100;

const cleanupR2 = async () => {
  console.log('Bắt đầu dọn dẹp R2 và MongoDB...');
  await connectDatabase();

  // 1. Dọn dẹp các bản ghi trong DB bị lỗi/treo (pending quá lâu hoặc failed)
  const oneDayAgo = new Date(Date.now() - 24 * 60 * 60 * 1000);
  const staleFiles = await FileModel.find({
    status: { $in: ['pending', 'failed'] },
    createdAt: { $lt: oneDayAgo }
  });

  console.log(`Tìm thấy ${staleFiles.length} file rác trong DB (treo upload hoặc lỗi). Bắt đầu xoá...`);
  let dbDeletedCount = 0;
  for (const file of staleFiles) {
    try {
      if (!file.externalUrl) {
        await r2Client.send(new DeleteObjectCommand({ Bucket: env.R2_BUCKET_NAME, Key: file.key }));
      }
      await file.deleteOne();
      dbDeletedCount++;
      console.log(`- Đã xoá file lỗi: ${file.name} (${file.key})`);
    } catch (err) {
      console.error(`Lỗi khi xoá file ${file.key}:`, err);
    }
  }
  console.log(`Đã xoá ${dbDeletedCount} file lỗi từ DB.\n`);

  // 2. Quét toàn bộ R2 để tìm file mồ côi (không có trong DB)
  console.log('Bắt đầu quét R2 để tìm file mồ côi...');
  let r2DeletedCount = 0;
  let isTruncated = true;
  let continuationToken: string | undefined = undefined;

  while (isTruncated) {
    const response: any = await r2Client.send(new ListObjectsV2Command({
      Bucket: env.R2_BUCKET_NAME,
      MaxKeys: BATCH_SIZE,
      ContinuationToken: continuationToken,
    }));

    if (response.Contents && response.Contents.length > 0) {
      for (const object of response.Contents) {
        if (!object.Key) continue;

        const existsInDb = await FileModel.exists({ key: object.Key, externalUrl: null });
        if (!existsInDb) {
          try {
            await r2Client.send(new DeleteObjectCommand({ Bucket: env.R2_BUCKET_NAME, Key: object.Key }));
            r2DeletedCount++;
            console.log(`- Đã xoá file mồ côi trên R2: ${object.Key}`);
          } catch (err) {
            console.error(`Lỗi khi xoá file mồ côi ${object.Key}:`, err);
          }
        }
      }
    }

    isTruncated = response.IsTruncated ?? false;
    continuationToken = response.NextContinuationToken;
  }

  console.log(`Đã xoá ${r2DeletedCount} file mồ côi trên R2.`);
  console.log('Hoàn tất dọn dẹp!');
  process.exit(0);
};

cleanupR2().catch((err) => {
  console.error('Lỗi nghiêm trọng khi dọn dẹp:', err);
  process.exit(1);
});
