//! Kiểm chứng cấu trúc DOM của index.html sau khi render.
//!
//! Widget tiến trình upload từng bị lồng trong `#orphans-modal` (thẻ đóng của
//! modal bị thiếu), nên `display: block` mà app.js đặt không có tác dụng: cha
//! vẫn `display: none` và người dùng không thấy tiến trình tải lên.

fn render_index() -> String {
    let mut jinja = minijinja::Environment::new();
    jinja.set_loader(minijinja::path_loader("templates"));
    jinja.add_filter("tojson", minijinja::filters::tojson);
    // Bản thật đăng ký `urlencode` riêng; ở test chỉ cần một filter cùng tên
    // để template parse được, giá trị không ảnh hưởng cấu trúc thẻ.
    jinja.add_filter("urlencode", |v: String| v);

    let template = jinja
        .get_template("index.html")
        .expect("index.html phải load được");

    template
        .render(minijinja::context! {
            user => minijinja::context! { email => "a@b.c", storageQuota => 0 },
            breadcrumb => Vec::<String>::new(),
            parentHref => None::<String>,
            items => Vec::<String>::new(),
            sortBy => "name",
            order => "asc",
            currentFolderId => None::<String>,
            allFolders => Vec::<String>::new(),
            currentPath => "/",
        })
        .expect("index.html phải render được")
}

/// Đếm độ sâu thẻ `div` tại vị trí `needle`, bỏ qua phần đứng sau nó.
/// `needle` phải trỏ tới đầu thẻ (`<div id="..."`), không phải tới attribute,
/// nếu không thẻ mở của chính element đó cũng bị tính vào độ sâu.
fn div_depth_at(html: &str, needle: &str) -> i32 {
    let cut = html
        .find(needle)
        .unwrap_or_else(|| panic!("phải tìm thấy {needle}"));
    let head = &html[..cut];

    let opens = head.matches("<div").count() as i32;
    let closes = head.matches("</div>").count() as i32;
    opens - closes
}

#[test]
fn the_div_trong_index_can_bang() {
    let html = render_index();
    let opens = html.matches("<div").count();
    let closes = html.matches("</div>").count();
    assert_eq!(opens, closes, "số <div> và </div> phải khớp");
}

#[test]
fn widget_upload_khong_nam_trong_modal_nao() {
    let html = render_index();

    // Widget phải ở ngay cấp <body>, tức không còn div nào đang mở.
    let depth = div_depth_at(&html, r#"<div id="global-upload-manager""#);
    assert_eq!(
        depth, 0,
        "widget upload phải là con trực tiếp của <body>, đang bị lồng {depth} cấp div"
    );
}

#[test]
fn modal_orphans_dong_truoc_widget_upload() {
    let html = render_index();

    let modal = html
        .find(r#"<div id="orphans-modal""#)
        .expect("phải có #orphans-modal");
    let widget = html
        .find(r#"<div id="global-upload-manager""#)
        .expect("phải có #global-upload-manager");

    assert!(modal < widget, "modal phải đứng trước widget trong markup");

    // Giữa hai mốc đó, modal (2 cấp: .modal và .modal-content) phải được đóng.
    let between = &html[modal..widget];
    let opens = between.matches("<div").count() as i32;
    let closes = between.matches("</div>").count() as i32;
    assert_eq!(
        opens - closes,
        0,
        "#orphans-modal và .modal-content phải đóng hết trước widget"
    );
}

#[test]
fn cac_id_app_js_can_deu_ton_tai() {
    let html = render_index();

    // app.js truy cập trực tiếp các id này; thiếu một cái là widget câm lặng.
    for id in [
        "global-upload-manager",
        "upload-manager-header",
        "upload-manager-body",
        "btn-toggle-upload-widget",
        "upload-completed-count",
        "upload-total-count",
        "upload-global-speed",
        "btn-upload",
        "file-input",
    ] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "thiếu id=\"{id}\" trong index.html"
        );
    }
}
