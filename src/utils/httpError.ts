export class HttpError extends Error {
  public readonly statusCode: number;

  constructor(statusCode: number, message: string) {
    super(message);
    this.statusCode = statusCode;
    this.name = 'HttpError';
  }
}

export const badRequest = (message: string) => new HttpError(400, message);
export const unauthorized = (message = 'Chưa đăng nhập hoặc token không hợp lệ') =>
  new HttpError(401, message);
export const forbidden = (message = 'Không có quyền thực hiện thao tác này') =>
  new HttpError(403, message);
export const notFound = (message: string) => new HttpError(404, message);
export const conflict = (message: string) => new HttpError(409, message);
