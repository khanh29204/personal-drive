# AGENTS.md

Quy tắc áp dụng cho mọi agent (Gemini, Claude, hoặc model khác) khi làm việc trong project này. Đọc file này trước khi thực hiện bất kỳ task nào.

## Ngôn ngữ

- Giao tiếp, comment trong code, commit message, tài liệu: **tiếng Việt** là mặc định.
- Chỉ dùng ngôn ngữ khác khi người dùng yêu cầu rõ ràng trong task cụ thể.

## Nguyên tắc làm việc

- **Trung thực, không bịa dữ liệu.** Nếu không chắc về API, cấu hình, hành vi thư viện, hoặc bất kỳ thông tin nào — dừng lại và hỏi thay vì đoán hoặc giả định.
- **Hỏi lại khi thiếu thông tin** cần thiết để hoàn thành task đúng ý, thay vì tự ý suy diễn rồi làm sai hướng. Khi hỏi, giải thích ngắn gọn tại sao thông tin đó ảnh hưởng đến cách làm.
- **Giải thích lý do/cách làm** khi thực hiện thay đổi quan trọng (kiến trúc, luồng dữ liệu, quyết định đánh đổi) — không chỉ note ngắn "đã sửa" mà không rõ vì sao.
- **Tập trung đúng phạm vi task được giao.** Không tự ý mở rộng, refactor ngoài yêu cầu, hoặc thêm tính năng chưa được hỏi.
- **Chỉ chủ động gợi ý cải tiến khi được cho phép.** Nếu thấy vấn đề tiềm ẩn ngoài phạm vi task, có thể nêu ngắn gọn ở cuối phản hồi nhưng không tự ý thực hiện nếu chưa được đồng ý.

## Quy tắc sửa file (bắt buộc)

- **Nếu file đã tồn tại: sửa trực tiếp (edit/patch) file đó.** Tuyệt đối không xoá file rồi tạo lại từ đầu, kể cả khi thay đổi lớn — trừ khi người dùng yêu cầu rõ ràng viết lại toàn bộ.
- Trước khi sửa, đọc lại nội dung file hiện tại để đảm bảo thay đổi khớp chính xác, tránh làm hỏng cấu trúc xung quanh.

## Chất lượng code (bắt buộc — người dùng là OCD với code)

- Mọi code TypeScript/JavaScript phải **sạch ESLint, format đúng Prettier** trước khi coi là hoàn thành. Chạy lint + format + build (nếu có) sau mỗi thay đổi, không để lỗi/cảnh báo tồn đọng không rõ lý do.
- Code phải **dễ đọc, có cấu trúc rõ ràng**: đặt tên biến/hàm có nghĩa, tách logic hợp lý theo layer sẵn có của project (không gộp business logic vào controller, không gọi thẳng DB/model ở nơi không phù hợp).
- Giữ đúng convention, pattern, cấu trúc thư mục đã có trong project — không tự ý đổi phong cách code giữa chừng.
- Không thêm dependency mới nếu không thực sự cần thiết; nếu cần, nêu rõ lý do trước khi thêm.

## Phạm vi công việc

Người dùng là full-stack developer (backend, frontend, mobile). Task có thể liên quan đến bất kỳ phần nào trong 3 mảng này — xác nhận rõ phạm vi (BE/FE/mobile hay cả 3) nếu task không nêu rõ.

## Thông tin project cụ thể: personal-drive-backend

### Stack

- Node.js + Express + TypeScript (biên dịch CommonJS, `module`/`moduleResolution: Node16`)
- MongoDB + Mongoose (ODM)
- Cloudflare R2 (lưu file, qua `@aws-sdk/client-s3` + `s3-request-presigner`, tương thích S3 API)
- EJS (server-side render cho client web) + CSS thuần (`src/public/style.css`)
- Validate input bằng `zod`
- Auth: verify token qua 1 trong 2 chiến lược (`AUTH_STRATEGY=local|api`), token nhận từ header `Authorization: Bearer` (mobile/API) hoặc cookie httpOnly (web/EJS) — xem `src/middlewares/extractToken.ts`

### Cấu trúc thư mục (`src/`)
config/       # env, kết nối MongoDB, khởi tạo S3Client cho R2
models/       # Mongoose schema (Folder, File)
middlewares/  # auth (bắt buộc login), optionalAuth (cho phép ẩn danh), error, extractToken
services/     # business logic thuần (auth, folder, file, r2) — KHÔNG đặt ở controller
controllers/  # nhận request, validate bằng zod, gọi service, trả response — KHÔNG chứa business logic
routes/       # khai báo route, gắn middleware + controller
views/        # file .ejs render cho client web
public/       # static assets (CSS, JS phía browser nếu có)
utils/        # helper dùng chung (asyncHandler, httpError, formatBytes, ...)
types/        # định nghĩa type mở rộng (vd Express Request)

### Quy tắc riêng của project này

- **Business logic luôn nằm ở `services/`**, không viết trực tiếp trong `controllers/`. Controller chỉ: validate input (`zod`) → gọi service → format response.
- **Không gọi `Model.find()`/`Model.create()` trực tiếp trong controller** — luôn qua service.
- Lỗi nghiệp vụ ném bằng các helper trong `utils/httpError.ts` (`badRequest`, `unauthorized`, `forbidden`, `notFound`, `conflict`), không `throw new Error()` trần trụi.
- Mọi route async phải bọc bằng `asyncHandler` (trong `utils/asyncHandler.ts`) để lỗi tự động chuyển tới `errorMiddleware`.
- Route `/api/*` luôn trả JSON khi lỗi; route web (EJS) render `views/error.ejs` khi lỗi — logic phân biệt đã có sẵn trong `error.middleware.ts`, không tự viết lại theo cách khác.
- Biến môi trường mới **phải khai báo và validate trong `src/config/env.ts` bằng `zod`**, đồng thời cập nhật `.env.example` kèm comment giải thích.
- Sau khi sửa `tsconfig.json`/thêm asset không phải `.ts` (view, css, ảnh...), kiểm tra `scripts/copy-assets.cjs` đã copy đúng sang `dist/` khi build chưa.
- Trước khi báo hoàn thành: chạy `npm run lint`, `npm run build` — phải sạch, không lỗi (warning cũ đã biết có thể bỏ qua nhưng phải nêu rõ đó là warning cũ, không phải mới phát sinh).