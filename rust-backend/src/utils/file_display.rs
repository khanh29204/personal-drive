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

pub fn get_file_icon(mime_type: &str) -> String {
    if mime_type.starts_with("image/") {
        "fa-file-image".to_string()
    } else if mime_type.starts_with("video/") {
        "fa-file-video".to_string()
    } else if mime_type.starts_with("audio/") {
        "fa-file-audio".to_string()
    } else if mime_type == "application/pdf" {
        "fa-file-pdf".to_string()
    } else if mime_type.contains("zip") || mime_type.contains("rar") || mime_type.contains("compressed") {
        "fa-file-archive".to_string()
    } else if mime_type.contains("word") {
        "fa-file-word".to_string()
    } else if mime_type.contains("excel") || mime_type.contains("spreadsheet") {
        "fa-file-excel".to_string()
    } else if mime_type.starts_with("text/") {
        "fa-file-alt".to_string()
    } else {
        "fa-file".to_string()
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
        assert_eq!(get_file_icon("image/png"), "fa-file-image");
        assert_eq!(get_file_icon("application/pdf"), "fa-file-pdf");
        assert_eq!(get_file_icon("unknown/mime"), "fa-file");
    }
}
