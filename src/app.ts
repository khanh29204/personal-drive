import path from 'node:path';

import cookieParser from 'cookie-parser';
import cors from 'cors';
import express, { json, static as static_ } from 'express';

import { env } from './config/env';
import { errorMiddleware, notFoundMiddleware } from './middlewares/error.middleware';
import authRoutes from './routes/auth.routes';
import fileRoutes from './routes/file.routes';
import folderRoutes from './routes/folder.routes';
import webRoutes from './routes/web.routes';

export const createApp = (): express.Express => {
  const app = express();

  app.set('view engine', 'ejs');
  app.set('views', path.join(__dirname, 'views'));

  app.use(cors({ origin: env.CORS_ORIGIN ?? true, credentials: true }));
  app.use(json());
  app.use(cookieParser());
  app.use(static_(path.join(__dirname, 'public')));

  app.get('/health', (_req, res) => {
    res.json({ status: 'ok' });
  });

  app.use('/api/auth', authRoutes);
  app.use('/api/folders', folderRoutes);
  app.use('/api/files', fileRoutes);
  app.use('/', webRoutes);

  app.use(notFoundMiddleware);
  app.use(errorMiddleware);

  return app;
};
