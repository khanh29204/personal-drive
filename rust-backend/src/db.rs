use mongodb::{bson::doc, options::IndexOptions, Client, Database, IndexModel};

pub async fn init_db(uri: &str) -> Result<Database, mongodb::error::Error> {
    let client = Client::with_uri_str(uri).await?;

    let db = client
        .default_database()
        .unwrap_or_else(|| client.database("personal_drive"));

    // Index cho collection `folders`
    let folders_col = db.collection::<mongodb::bson::Document>("folders");
    folders_col
        .create_index(
            IndexModel::builder()
                .keys(doc! { "ownerId": 1, "parentId": 1 })
                .build(),
        )
        .await?;

    folders_col
        .create_index(
            IndexModel::builder()
                .keys(doc! { "parentId": 1 })
                .build(),
        )
        .await?;

    folders_col
        .create_index(
            IndexModel::builder()
                .keys(doc! { "ownerId": 1 })
                .build(),
        )
        .await?;

    // Index cho collection `files`
    let files_col = db.collection::<mongodb::bson::Document>("files");
    files_col
        .create_index(
            IndexModel::builder()
                .keys(doc! { "key": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;

    files_col
        .create_index(
            IndexModel::builder()
                .keys(doc! { "ownerId": 1, "folderId": 1, "status": 1 })
                .build(),
        )
        .await?;

    files_col
        .create_index(
            IndexModel::builder()
                .keys(doc! { "folderId": 1 })
                .build(),
        )
        .await?;

    files_col
        .create_index(
            IndexModel::builder()
                .keys(doc! { "ownerId": 1 })
                .build(),
        )
        .await?;

    Ok(db)
}
