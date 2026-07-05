import type { CookieOptions } from 'express';

import { env } from '../config/env';

/**
 * Options cookie dùng chung khi set/clear token cookie.
 * httpOnly: JS phía client không đọc được, chống XSS đánh cắp token.
 * sameSite 'lax': đủ để chặn CSRF cơ bản khi cookie chỉ dùng cho GET điều hướng
 * và các request cùng site; nếu sau này có form POST cross-site cần xem lại.
 */
export const buildAuthCookieOptions = (): CookieOptions => ({
  httpOnly: true,
  secure: env.COOKIE_SECURE,
  sameSite: 'lax',
  domain: env.COOKIE_DOMAIN || undefined,
  maxAge: env.COOKIE_MAX_AGE_DAYS * 24 * 60 * 60 * 1000,
  path: '/',
});
