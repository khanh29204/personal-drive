import type { Request } from 'express';

import { env } from '../config/env';

/**
 * Lấy token từ 1 trong 2 nguồn:
 * 1. Header `Authorization: Bearer <token>` — ưu tiên, dùng cho mobile/SPA/API client.
 * 2. Cookie httpOnly (COOKIE_NAME) — dùng cho client web render bằng EJS, được set
 *    qua endpoint POST /api/auth/login của chính backend này.
 */
export const extractBearerToken = (req: Request): string | null => {
  const header = req.headers.authorization;
  if (header?.startsWith('Bearer ')) {
    const token = header.slice('Bearer '.length).trim();
    if (token.length > 0) {
      return token;
    }
  }

  const cookieToken = req.cookies?.[env.COOKIE_NAME];
  return typeof cookieToken === 'string' && cookieToken.length > 0 ? cookieToken : null;
};
