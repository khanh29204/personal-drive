import { createApp } from './app';
import { connectDatabase } from './config/db';
import { env } from './config/env';

const start = async (): Promise<void> => {
  await connectDatabase();

  const app = createApp();

  app.listen(env.PORT, () => {
    // eslint-disable-next-line no-console
    console.log(`🚀 Server đang chạy tại http://localhost:${env.PORT}`);
  });
};

start().catch((error) => {
  // eslint-disable-next-line no-console
  console.error('❌ Không thể khởi động server:', error);
  process.exit(1);
});
