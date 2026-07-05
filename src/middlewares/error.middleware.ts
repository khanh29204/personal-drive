import type { NextFunction, Request, Response } from 'express';

import { HttpError } from '../utils/httpError';

export const notFoundMiddleware = (req: Request, res: Response): void => {
  const message = `Không tìm thấy route ${req.method} ${req.originalUrl}`;
  if (req.path.startsWith('/api')) {
    res.status(404).json({ message });
    return;
  }
  res.status(404).render('error', { message });
};

const respondError = (req: Request, res: Response, statusCode: number, message: string): void => {
  if (req.path.startsWith('/api')) {
    res.status(statusCode).json({ message });
    return;
  }
  res.status(statusCode).render('error', { message });
};

export const errorMiddleware = (
  error: unknown,
  req: Request,
  res: Response,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  _next: NextFunction,
): void => {
  if (error instanceof HttpError) {
    respondError(req, res, error.statusCode, error.message);
    return;
  }

  // eslint-disable-next-line no-console
  console.error('Lỗi không xác định:', error);
  respondError(req, res, 500, 'Đã có lỗi xảy ra ở server');
};
