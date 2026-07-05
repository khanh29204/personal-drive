import type { Request, Response } from 'express';
import { z } from 'zod';

import { env } from '../config/env';
import * as authService from '../services/auth.service';
import { buildAuthCookieOptions } from '../utils/authCookie';

const loginBodySchema = z.object({
  userName: z.string().trim().min(1, 'userName không được để trống'),
  password: z.string().min(1, 'password không được để trống'),
});

/**
 * Đăng nhập dành cho client web (EJS): nhận userName/password, proxy sang
 * auth microservice để lấy token, rồi set token vào httpOnly cookie thay vì
 * trả token trong JSON response — tránh lộ token cho JS phía client (chống XSS).
 * Mobile/SPA không dùng endpoint này, họ gọi thẳng auth microservice và tự
 * gắn Bearer token vào header.
 */
export const login = async (req: Request, res: Response): Promise<void> => {
  const { userName, password } = loginBodySchema.parse(req.body);
  const token = await authService.loginViaApi(userName, password);
  res.cookie(env.COOKIE_NAME, token, buildAuthCookieOptions());
  res.json({ message: 'Đăng nhập thành công' });
};

export const logout = (_req: Request, res: Response): void => {
  res.clearCookie(env.COOKIE_NAME, { ...buildAuthCookieOptions(), maxAge: undefined });
  res.json({ message: 'Đã đăng xuất' });
};
