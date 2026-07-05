import 'dotenv/config';

import { z } from 'zod';

const envSchema = z.object({
  PORT: z.coerce.number().default(4000),
  NODE_ENV: z.enum(['development', 'production', 'test']).default('development'),

  MONGODB_URI: z.string().min(1, 'MONGODB_URI là bắt buộc'),

  AUTH_STRATEGY: z.enum(['local', 'api']).default('local'),
  JWT_SECRET: z.string().min(1, 'JWT_SECRET là bắt buộc'),
  AUTH_API_BASE_URL: z.string().url('AUTH_API_BASE_URL phải là URL hợp lệ'),

  COOKIE_NAME: z.string().default('drive_token'),
  COOKIE_DOMAIN: z.string().optional(),
  COOKIE_SECURE: z.coerce.boolean().default(false),
  COOKIE_MAX_AGE_DAYS: z.coerce.number().default(7),

  // Origin cụ thể của frontend (nếu có, để bật CORS credentials cho cookie).
  // Để trống thì reflect origin của request (chấp nhận mọi origin nhưng vẫn
  // hoạt động đúng với credentials, phù hợp khi chỉ dùng cá nhân).
  CORS_ORIGIN: z.string().optional(),

  R2_ACCOUNT_ID: z.string().min(1, 'R2_ACCOUNT_ID là bắt buộc'),
  R2_ACCESS_KEY_ID: z.string().min(1, 'R2_ACCESS_KEY_ID là bắt buộc'),
  R2_SECRET_ACCESS_KEY: z.string().min(1, 'R2_SECRET_ACCESS_KEY là bắt buộc'),
  R2_BUCKET_NAME: z.string().min(1, 'R2_BUCKET_NAME là bắt buộc'),
  R2_UPLOAD_URL_EXPIRES_IN: z.coerce.number().default(300),
  R2_DOWNLOAD_URL_EXPIRES_IN: z.coerce.number().default(3600),
  R2_PUBLIC_DOMAIN: z.string().url('R2_PUBLIC_DOMAIN phải là URL hợp lệ, vd: https://cdn...').optional(),
});

const parsed = envSchema.safeParse(process.env);

if (!parsed.success) {
  // eslint-disable-next-line no-console
  console.error('❌ Biến môi trường không hợp lệ:', parsed.error.flatten().fieldErrors);
  throw new Error('Cấu hình biến môi trường không hợp lệ, kiểm tra lại file .env');
}

export const env = parsed.data;
