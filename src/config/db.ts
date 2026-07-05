import mongoose from 'mongoose';

import { env } from './env';

export const connectDatabase = async (): Promise<void> => {
  mongoose.set('strictQuery', true);

  await mongoose.connect(env.MONGODB_URI);

  // eslint-disable-next-line no-console
  console.log('✅ Đã kết nối MongoDB');

  mongoose.connection.on('error', (error) => {
    // eslint-disable-next-line no-console
    console.error('❌ Lỗi kết nối MongoDB:', error);
  });
};
