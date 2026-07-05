import type { NextFunction, Request, Response } from 'express';

import { verifyToken } from '../services/auth.service';
import { unauthorized } from '../utils/httpError';

import { extractBearerToken } from './extractToken';

/**
 * Dùng cho các route bắt buộc đăng nhập (upload, tạo folder, xoá, ...).
 * Nếu không có token hoặc token không hợp lệ -> 401.
 */
export const authMiddleware = async (
  req: Request,
  _res: Response,
  next: NextFunction,
): Promise<void> => {
  try {
    const token = extractBearerToken(req);
    if (!token) {
      throw unauthorized('Thiếu token xác thực');
    }
    req.user = await verifyToken(token);
    next();
  } catch (error) {
    next(error);
  }
};
