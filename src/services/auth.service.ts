import axios, { AxiosError } from 'axios';
import { verify } from 'jsonwebtoken';

import { env } from '../config/env';
import type { AuthenticatedUser } from '../types/express';
import { unauthorized } from '../utils/httpError';

interface TokenPayload {
  id?: string;
  _id?: string;
  userId?: string;
  user_name: string;
  iat: number;
}

/**
 * Verify token bằng cách tự giải mã JWT với secret dùng chung giữa backend này
 * và auth microservice (yêu cầu 2 bên ký cùng JWT_SECRET, cùng thuật toán HS256).
 * Nhanh, không phụ thuộc network, nhưng không phát hiện được token bị revoke
 * phía auth service (vd logout/khoá tài khoản) trước khi token hết hạn.
 */
const verifyLocal = (token: string): AuthenticatedUser => {
  try {
    const decoded = verify(token, env.JWT_SECRET) as TokenPayload;
    const userId = decoded.id || decoded._id || decoded.userId;
    if (!userId) throw new Error('Invalid token format');
    return { id: userId, userName: decoded.user_name };
  } catch {
    throw unauthorized();
  }
};

/**
 * Verify token bằng cách gọi API POST /auth/checkToken của auth microservice.
 * Chậm hơn (thêm 1 network call mỗi request) nhưng luôn đồng bộ trạng thái
 * thật của token phía auth service.
 */
const verifyViaApi = async (token: string): Promise<AuthenticatedUser> => {
  try {
    const { data } = await axios.post<TokenPayload>(`${env.AUTH_API_BASE_URL}/auth/checkToken`, {
      token,
    });
    const userId = data.id || data._id || data.userId;
    if (!userId) throw new Error('Invalid token format');
    return { id: userId, userName: data.user_name };
  } catch (error) {
    if (error instanceof AxiosError && error.response?.status === 401) {
      throw unauthorized();
    }
    throw error;
  }
};

/**
 * Điểm vào duy nhất để verify token, chọn chiến lược theo env AUTH_STRATEGY.
 * Muốn đổi chiến lược chỉ cần đổi biến môi trường, không cần sửa code nơi khác gọi.
 */
export const verifyToken = (token: string): Promise<AuthenticatedUser> | AuthenticatedUser => {
  if (env.AUTH_STRATEGY === 'api') {
    return verifyViaApi(token);
  }
  return verifyLocal(token);
};

/**
 * Proxy đăng nhập sang auth microservice để lấy token, dùng riêng cho endpoint
 * /api/auth/login của backend này (mục đích: nhận token rồi set httpOnly cookie
 * cho client web/EJS). Mobile/SPA vẫn có thể đăng nhập trực tiếp với auth
 * microservice và tự gắn Bearer token, không bắt buộc phải qua hàm này.
 */
export const loginViaApi = async (userName: string, password: string): Promise<string> => {
  try {
    const { data } = await axios.post<{ token: string }>(`${env.AUTH_API_BASE_URL}/auth/login`, {
      userName,
      password,
    });
    return data.token;
  } catch (error) {
    if (error instanceof AxiosError && error.response?.status === 401) {
      throw unauthorized('Sai tên đăng nhập hoặc mật khẩu');
    }
    throw error;
  }
};
