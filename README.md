# Personal Drive Backend

Backend Node.js/Express/TypeScript cho drive cá nhân, lưu trữ file trên Cloudflare R2, metadata trên MongoDB.

## Cài đặt

```bash
npm install
cp .env.example .env   # rồi điền các giá trị thật
npm run dev             # chạy dev với hot-reload
npm run build && npm start  # build & chạy production
```

## Xác thực

Backend hỗ trợ **song song 2 cách** truyền token, ưu tiên header trước:

- **Mobile/SPA**: gửi `Authorization: Bearer <token>` (token lấy trực tiếp từ
  `POST {AUTH_API_BASE_URL}/auth/login` của auth microservice, không qua backend này).
- **Web (EJS)**: gọi `POST /api/auth/login` với `{ userName, password }`. Backend
  proxy sang auth microservice lấy token, rồi tự set vào **httpOnly cookie**
  (tên cookie theo `COOKIE_NAME`) — token không lộ ra cho JS phía client, chống
  đánh cắp qua XSS. Gọi `POST /api/auth/logout` để xoá cookie.

`AUTH_STRATEGY`: `local` (mặc định) hoặc `api`.

- `local`: backend tự verify JWT bằng `JWT_SECRET` dùng chung với auth microservice.
  Yêu cầu 2 bên ký token cùng secret, cùng thuật toán (mặc định HS256).
- `api`: backend gọi `POST {AUTH_API_BASE_URL}/auth/checkToken` với body `{ token }`
  mỗi request để verify. Chỉ cần đổi biến này, không cần sửa code.

## Biến môi trường quan trọng

- `R2_*`: thông tin Cloudflare R2 (Account ID, Access Key, Secret Key, tên bucket).
- `COOKIE_SECURE`: bật `true` khi chạy production qua HTTPS (bắt buộc để cookie
  hoạt động đúng, vì browser chặn cookie `Secure` trên HTTP).
- `CORS_ORIGIN`: điền domain thật của frontend nếu deploy production, để trống
  thì backend reflect origin của request (đủ dùng cho cá nhân/dev).

## Luồng upload file

1. Client gọi `POST /api/files/upload-url` (yêu cầu đăng nhập) với
   `{ name, mimeType, size, folderId, isPublic }`.
2. Backend tạo record `File` với `status: "pending"`, sinh presigned PUT URL, trả về
   `{ fileId, uploadUrl }`.
3. Client `PUT` file thẳng lên `uploadUrl`, **không qua backend**.
4. Client gọi `POST /api/files/:fileId/complete` (yêu cầu đăng nhập) để xác nhận.
   Backend kiểm tra object đã tồn tại thật trên R2 (HeadObject) rồi mới đánh dấu
   `status: "completed"`. Nếu chưa tồn tại → `status: "failed"` và trả lỗi 400.

## API endpoints

| Method | Endpoint                          | Auth      | Mô tả                                   |
| ------ | ---------------------------------- | --------- | ---------------------------------------- |
| POST   | /api/auth/login                    | none      | Đăng nhập web: proxy sang auth microservice, set httpOnly cookie |
| POST   | /api/auth/logout                   | none      | Xoá cookie đăng nhập                     |
| GET    | /api/folders?parentId=             | optional  | List folder (public + của mình nếu login)|
| POST   | /api/folders                       | required  | Tạo folder                               |
| PATCH  | /api/folders/:id                   | required  | Sửa tên/di chuyển/đổi public (chủ sở hữu)|
| DELETE | /api/folders/:id                   | required  | Xoá đệ quy folder + file con trên R2     |
| GET    | /api/files?folderId=                | optional  | List file (public + của mình nếu login)  |
| POST   | /api/files/upload-url               | required  | Xin presigned PUT URL                    |
| POST   | /api/files/:id/complete             | required  | Xác nhận upload hoàn tất                 |
| GET    | /api/files/:id/download-url          | optional  | Xin presigned GET URL để tải             |
| DELETE | /api/files/:id                      | required  | Xoá file (DB + object trên R2)           |

`optional`: route dùng `optionalAuthMiddleware` — không đăng nhập vẫn xem được nội dung public,
có token hợp lệ thì thấy thêm nội dung private của chính mình.

## Việc cần làm khi tích hợp thật

- Đảm bảo bucket R2 **không** bật public access ở cấp bucket — mọi truy cập file
  (kể cả public) đều đi qua presigned URL do backend sinh ra, tránh lộ credentials
  hoặc phải cấu hình CORS phức tạp trên R2.
- Nếu chọn `AUTH_STRATEGY=local`, cần đồng bộ `JWT_SECRET` với auth microservice tại
  `https://file.quockhanh020924.id.vn` và xác nhận thuật toán ký token là HS256.
- Cấu hình CORS trên bucket R2 để cho phép domain frontend gọi PUT trực tiếp.
