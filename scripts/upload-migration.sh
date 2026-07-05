#!/bin/bash

# ==============================================================================
# Script upload toàn bộ file/folder lên Personal Drive (chuyển đổi VPS sang R2)
# Yêu cầu cài đặt trên VPS: curl, jq, file
# ==============================================================================

# Cấu hình mặc định (bạn có thể thay đổi ở đây hoặc truyền qua tham số)
API_URL="http://localhost:4000"
USERNAME="your_username"
PASSWORD="your_password"
TARGET_DIR="."

# Ghi đè cấu hình nếu có tham số truyền vào
API_URL=${1:-$API_URL}
USERNAME=${2:-$USERNAME}
PASSWORD=${3:-$PASSWORD}
TARGET_DIR=${4:-$TARGET_DIR}

# Xóa dấu / ở cuối TARGET_DIR để xử lý chuỗi dễ hơn
TARGET_DIR="${TARGET_DIR%/}"

echo "🚀 Bắt đầu quá trình upload từ: $TARGET_DIR"
echo "🌍 Server API: $API_URL"

# Kiểm tra công cụ cần thiết
for cmd in curl jq file; do
  if ! command -v $cmd &> /dev/null; then
    echo "❌ Lỗi: Cần cài đặt '$cmd' để chạy script này (VD: sudo apt install $cmd)"
    exit 1
  fi
done

echo -n "🔑 Đang đăng nhập... "
LOGIN_RES=$(curl -s -X POST "$API_URL/api/auth/login?token=true" \
  -H "Content-Type: application/json" \
  -d "{\"userName\": \"$USERNAME\", \"password\": \"$PASSWORD\"}")

# Kiểm tra nếu server trả về lỗi
if echo "$LOGIN_RES" | grep -q '"message"' && ! echo "$LOGIN_RES" | grep -q '"Đăng nhập thành công"'; then
  echo "Thất bại!"
  echo "Lỗi: $LOGIN_RES"
  exit 1
fi

# Lấy giá trị token từ JSON response
TOKEN=$(echo "$LOGIN_RES" | jq -r '.token // empty')

if [ -z "$TOKEN" ]; then
  echo "Thất bại! Không tìm thấy token trong phản hồi trả về."
  exit 1
fi
echo "Thành công!"

# Sử dụng mảng kết hợp (associative array) để map đường dẫn local -> ID thư mục trên API
# Yêu cầu Bash >= 4.0
declare -A FOLDER_MAP
FOLDER_MAP["."]="null" # Gốc mặc định là null

# Hàm đệ quy tạo thư mục
create_folder() {
  local local_path="$1"
  local parent_path="${local_path%/*}"
  local folder_name="${local_path##*/}"
  
  # Nếu không có dấu / (tức là thư mục nằm trực tiếp ở gốc)
  if [ "$parent_path" = "$local_path" ]; then
    parent_path="."
  fi

  local parent_id="${FOLDER_MAP[$parent_path]}"

  # Nếu thư mục cha chưa được tạo (có thể do thứ tự sort hoặc thư mục lồng sâu), tạo nó trước
  if [ -z "$parent_id" ]; then
    create_folder "$parent_path"
    parent_id="${FOLDER_MAP[$parent_path]}"
  fi

  local parent_payload="null"
  if [ "$parent_id" != "null" ]; then
    parent_payload="\"$parent_id\""
  fi

  local res
  res=$(curl -s -X POST "$API_URL/api/folders" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"name\": \"$folder_name\", \"parentId\": $parent_payload, \"isPublic\": false}")
  
  local folder_id
  folder_id=$(echo "$res" | jq -r '._id // empty')
  
  if [ -z "$folder_id" ]; then
    echo "❌ Lỗi khi tạo thư mục '$local_path': $res" >&2
    exit 1
  fi

  FOLDER_MAP["$local_path"]="$folder_id"
  echo "📁 Đã tạo thư mục: $local_path" >&2
}

echo "=========================================="
echo "📂 ĐANG QUÉT VÀ TẠO CẤU TRÚC THƯ MỤC"
echo "=========================================="

# Tìm tất cả thư mục con (bỏ qua bản thân TARGET_DIR), sort để đảm bảo thư mục cha đứng trước con
# Sử dụng process substitution < <(...) để không tạo subshell (tránh mất biến FOLDER_MAP)
while IFS= read -r dir; do
  # Bỏ qua nếu dir rỗng (trường hợp TARGET_DIR trống)
  if [ -z "$dir" ]; then continue; fi

  # Tính đường dẫn tương đối
  rel_path="${dir#$TARGET_DIR/}"
  if [ "$rel_path" = "$dir" ]; then rel_path="${dir#$TARGET_DIR}"; fi
  rel_path="${rel_path#/}"
  
  if [ -n "$rel_path" ]; then
    create_folder "$rel_path"
  fi
done < <(find "$TARGET_DIR" -mindepth 1 -type d | sort)


echo "=========================================="
echo "☁️ ĐANG UPLOAD TẬP TIN"
echo "=========================================="

while IFS= read -r file; do
  # Bỏ qua script này nếu vô tình nằm trong thư mục đích
  if [[ "$file" == *"$0"* ]]; then
    continue
  fi

  rel_path="${file#$TARGET_DIR/}"
  if [ "$rel_path" = "$file" ]; then rel_path="${file#$TARGET_DIR}"; fi
  rel_path="${rel_path#/}"
  
  parent_dir="${rel_path%/*}"
  file_name="${rel_path##*/}"
  
  if [ "$parent_dir" = "$rel_path" ]; then
    parent_dir="."
  fi

  folder_id="${FOLDER_MAP[$parent_dir]}"
  if [ -z "$folder_id" ]; then
    folder_id="null"
  fi

  # Lấy thông tin file
  mime_type=$(file -b --mime-type "$file")
  # Tùy hệ điều hành (Linux/Mac) thì wc hoặc stat khác nhau, dùng wc an toàn hơn
  size=$(wc -c < "$file" | tr -d ' ')

  # Chuẩn bị payload lấy URL (Dùng jq để tự động escape các ký tự đặc biệt trong tên file)
  folder_payload="null"
  if [ "$folder_id" != "null" ]; then
    folder_payload="\"$folder_id\""
  fi

  payload=$(jq -n \
    --arg name "$file_name" \
    --arg mime "$mime_type" \
    --argjson size "$size" \
    --argjson fid "$folder_payload" \
    '{name: $name, mimeType: $mime, size: $size, folderId: $fid, isPublic: false}')

  echo "⏳ Đang xử lý: $rel_path ($size bytes) ..."
  
  # Bước 1: Xin upload url
  res=$(curl -s -X POST "$API_URL/api/files/upload-url" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "$payload")

  file_api_id=$(echo "$res" | jq -r '.fileId // empty')
  upload_url=$(echo "$res" | jq -r '.uploadUrl // empty')

  if [ -z "$file_api_id" ] || [ -z "$upload_url" ]; then
    echo "  ❌ Lỗi khi lấy Upload URL: $res"
    continue
  fi

  # Bước 2: Upload trực tiếp lên R2
  http_code=$(curl -s -o /dev/null -w "%{http_code}" -X PUT -T "$file" \
    -H "Content-Type: $mime_type" "$upload_url")
  
  if [ "$http_code" -lt 200 ] || [ "$http_code" -ge 300 ]; then
    echo "  ❌ Lỗi khi upload lên R2 (HTTP $http_code)"
    continue
  fi

  # Bước 3: Xác nhận thành công
  complete_res=$(curl -s -X POST "$API_URL/api/files/$file_api_id/complete" \
    -H "Authorization: Bearer $TOKEN")
  echo "  ✅ Hoàn tất!"

done < <(find "$TARGET_DIR" -type f | sort)

echo "🎉 TOÀN BỘ QUÁ TRÌNH UPLOAD ĐÃ KẾT THÚC!"
