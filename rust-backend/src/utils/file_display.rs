pub fn format_bytes(bytes: i64) -> String {
    if bytes <= 0 {
        return "0 B".to_string();
    }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let bytes_f = bytes as f64;
    let exponent = (bytes_f.log2() / 10.0).floor() as usize;
    let exponent = exponent.min(units.len() - 1);
    let value = bytes_f / (1024_f64.powi(exponent as i32));

    if exponent == 0 {
        format!("{:.0} {}", value, units[exponent])
    } else {
        format!("{:.1} {}", value, units[exponent])
    }
}

fn get_extension(name: &str) -> String {
    if let Some(pos) = name.rfind('.') {
        name[pos + 1..].trim().to_lowercase()
    } else {
        String::new()
    }
}

pub fn get_file_icon(name: &str, mime_type: &str, is_linked: bool) -> String {
    if is_linked {
        return "fa-link".to_string();
    }

    let ext = get_extension(name);

    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff" | "heic" | "psd" | "ai" => {
            return "fa-file-image".to_string();
        }
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "3gp" | "mpg" | "mpeg" => {
            return "fa-file-video".to_string();
        }
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" | "mid" | "midi" => {
            return "fa-file-audio".to_string();
        }
        "pdf" => return "fa-file-pdf".to_string(),
        "doc" | "docx" | "odt" | "rtf" => return "fa-file-word".to_string(),
        "xls" | "xlsx" | "csv" | "ods" => return "fa-file-excel".to_string(),
        "ppt" | "pptx" | "odp" => return "fa-file-powerpoint".to_string(),
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" | "dmg" | "apk" | "cab" => {
            return "fa-file-archive".to_string();
        }
        "js" | "ts" | "jsx" | "tsx" | "html" | "htm" | "css" | "json" | "py" | "rs" | "cpp" | "c"
        | "h" | "hpp" | "java" | "php" | "sh" | "bat" | "cmd" | "sql" | "yaml" | "yml" | "xml"
        | "md" | "go" | "kt" | "swift" | "rb" => return "fa-file-code".to_string(),
        "exe" | "msi" | "deb" | "rpm" | "app" => return "fa-cogs".to_string(),
        _ => {}
    }

    if mime_type.starts_with("image/") {
        "fa-file-image".to_string()
    } else if mime_type.starts_with("video/") {
        "fa-file-video".to_string()
    } else if mime_type.starts_with("audio/") {
        "fa-file-audio".to_string()
    } else if mime_type == "application/pdf" {
        "fa-file-pdf".to_string()
    } else if mime_type.contains("zip")
        || mime_type.contains("rar")
        || mime_type.contains("compressed")
        || mime_type.contains("7z")
        || mime_type.contains("tar")
    {
        "fa-file-archive".to_string()
    } else if mime_type.contains("word") || mime_type.contains("wordprocessingml") {
        "fa-file-word".to_string()
    } else if mime_type.contains("excel") || mime_type.contains("spreadsheetml") {
        "fa-file-excel".to_string()
    } else if mime_type.contains("powerpoint") || mime_type.contains("presentationml") {
        "fa-file-powerpoint".to_string()
    } else if mime_type.starts_with("text/")
        || mime_type.contains("json")
        || mime_type.contains("javascript")
        || mime_type.contains("xml")
    {
        "fa-file-alt".to_string()
    } else {
        "fa-file".to_string()
    }
}

pub fn get_file_category_label(name: &str, mime_type: &str, is_linked: bool) -> String {
    if is_linked {
        return "Liên kết ngoài".to_string();
    }

    let ext = get_extension(name);

    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff" | "heic" | "psd" | "ai" => {
            return "Hình ảnh".to_string();
        }
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "3gp" | "mpg" | "mpeg" => {
            return "Video".to_string();
        }
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" | "mid" | "midi" => {
            return "Âm thanh".to_string();
        }
        "pdf" => return "Tài liệu PDF".to_string(),
        "doc" | "docx" | "odt" | "rtf" => return "Tài liệu Word".to_string(),
        "xls" | "xlsx" | "csv" | "ods" => return "Bảng tính Excel".to_string(),
        "ppt" | "pptx" | "odp" => return "Trình chiếu PPT".to_string(),
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" | "dmg" | "apk" | "cab" => {
            return "Tệp nén".to_string();
        }
        "js" | "ts" | "jsx" | "tsx" | "html" | "htm" | "css" | "json" | "py" | "rs" | "cpp" | "c"
        | "h" | "hpp" | "java" | "php" | "sh" | "bat" | "cmd" | "sql" | "yaml" | "yml" | "xml"
        | "md" | "go" | "kt" | "swift" | "rb" => return "Mã nguồn / Code".to_string(),
        "exe" | "msi" | "deb" | "rpm" | "app" => return "Thực thi / App".to_string(),
        _ => {}
    }

    if mime_type.starts_with("image/") {
        "Hình ảnh".to_string()
    } else if mime_type.starts_with("video/") {
        "Video".to_string()
    } else if mime_type.starts_with("audio/") {
        "Âm thanh".to_string()
    } else if mime_type == "application/pdf" {
        "Tài liệu PDF".to_string()
    } else if mime_type.contains("word") || mime_type.contains("wordprocessingml") {
        "Tài liệu Word".to_string()
    } else if mime_type.contains("excel") || mime_type.contains("spreadsheetml") {
        "Bảng tính Excel".to_string()
    } else if mime_type.contains("powerpoint") || mime_type.contains("presentationml") {
        "Trình chiếu PPT".to_string()
    } else if mime_type.contains("zip")
        || mime_type.contains("rar")
        || mime_type.contains("tar")
        || mime_type.contains("7z")
        || mime_type.contains("compressed")
    {
        "Tệp nén".to_string()
    } else if mime_type.starts_with("text/")
        || mime_type.contains("json")
        || mime_type.contains("javascript")
        || mime_type.contains("xml")
    {
        "Văn bản / Code".to_string()
    } else {
        "Tệp".to_string()
    }
}

pub fn get_file_category_code(name: &str, mime_type: &str, is_linked: bool) -> &'static str {
    if is_linked {
        return "link";
    }

    let ext = get_extension(name);

    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff" | "heic" | "psd" | "ai" => {
            return "image";
        }
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "3gp" | "mpg" | "mpeg" => {
            return "video";
        }
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" | "mid" | "midi" => {
            return "audio";
        }
        "pdf" | "doc" | "docx" | "odt" | "rtf" | "xls" | "xlsx" | "csv" | "ods" | "ppt" | "pptx" | "odp" => {
            return "doc";
        }
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" | "dmg" | "apk" | "cab" => {
            return "archive";
        }
        "js" | "ts" | "jsx" | "tsx" | "html" | "htm" | "css" | "json" | "py" | "rs" | "cpp" | "c"
        | "h" | "hpp" | "java" | "php" | "sh" | "bat" | "cmd" | "sql" | "yaml" | "yml" | "xml"
        | "md" | "go" | "kt" | "swift" | "rb" => return "doc",
        _ => {}
    }

    if mime_type.starts_with("image/") {
        "image"
    } else if mime_type.starts_with("video/") {
        "video"
    } else if mime_type.starts_with("audio/") {
        "audio"
    } else if mime_type == "application/pdf"
        || mime_type.contains("word")
        || mime_type.contains("excel")
        || mime_type.contains("powerpoint")
        || mime_type.contains("spreadsheet")
        || mime_type.contains("presentation")
        || mime_type.starts_with("text/")
        || mime_type.contains("json")
        || mime_type.contains("javascript")
    {
        "doc"
    } else if mime_type.contains("zip")
        || mime_type.contains("rar")
        || mime_type.contains("tar")
        || mime_type.contains("7z")
        || mime_type.contains("compressed")
    {
        "archive"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn test_get_file_icon() {
        assert_eq!(get_file_icon("photo.png", "application/octet-stream", false), "fa-file-image");
        assert_eq!(get_file_icon("doc.pdf", "application/octet-stream", false), "fa-file-pdf");
        assert_eq!(get_file_icon("Winrar.exe", "application/octet-stream", false), "fa-cogs");
        assert_eq!(get_file_icon("link", "", true), "fa-link");
    }

    #[test]
    fn test_get_file_category_label() {
        assert_eq!(get_file_category_label("photo.png", "application/octet-stream", false), "Hình ảnh");
        assert_eq!(get_file_category_label("movie.mkv", "application/octet-stream", false), "Video");
        assert_eq!(get_file_category_label("data.sql", "application/octet-stream", false), "Mã nguồn / Code");
        assert_eq!(get_file_category_label("app.apk", "application/octet-stream", false), "Tệp nén");
    }

    #[test]
    fn test_get_file_category_code() {
        assert_eq!(get_file_category_code("photo.png", "", false), "image");
        assert_eq!(get_file_category_code("movie.mkv", "", false), "video");
        assert_eq!(get_file_category_code("anything", "", true), "link");
    }
}
