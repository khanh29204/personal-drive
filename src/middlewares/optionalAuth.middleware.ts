import type { NextFunction, Request, Response } from 'express';

import { verifyToken } from '../services/auth.service';

import { extractBearerToken } from './extractToken';

/**
 * Dùng cho các route xem/tải file: cho phép người chưa đăng nhập tiếp tục
 * (chỉ thấy nội dung public), nếu có token hợp lệ thì gắn req.user để
 * controller có thể trả thêm nội dung private của chính user đó.
 * Token sai/hết hạn KHÔNG chặn request, chỉ bỏ qua như chưa đăng nhập.
 */
export const optionalAuthMiddleware = async (
  req: Request,
  _res: Response,
  next: NextFunction,
): Promise<void> => {
  const token = extractBearerToken(req);
  if (!token) {
    next();
    return;
  }

  try {
    req.user = await verifyToken(token);
  } catch {
    // Token không hợp lệ -> coi như khách chưa đăng nhập, không throw lỗi
  }
  next();
};
