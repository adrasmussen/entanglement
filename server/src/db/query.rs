use anyhow;

use async_trait::async_trait;

use mysql_async::{prelude::*, BinaryProtocol, ResultSetStream};

use crate::db::{ESDbConn, ESDbQuery};

type StreamReturn<T> =
    anyhow::Result<Option<ResultSetStream<'static, 'static, 'static, T, BinaryProtocol>>>;

struct ImagePerms {
    user: String,
    image: String,
    mode: u32,
}

impl ESDbConn for mysql_async::Conn {}

#[async_trait]
impl ESDbQuery<mysql_async::Conn> for ImagePerms {
    type QueryOutput = bool;

    async fn result_stream(self, conn: mysql_async::Conn) -> StreamReturn<Self::QueryOutput> {
        let result = r"
            SELECT EXISTS(
            SELECT TOP 1
            (SELECT album FROM albums WHERE user = :user AND mode >= :mode) allowed_albums
            INNER JOIN
            (SELECT image, album from album_contents WHERE image = :image) albums_with_image
            ON (allowed_albums.album = albums_with_image.album)
            )
        "
        .with(params! {
          "user" => self.user,
          "image" => self.image,
          "mode" => self.mode,
        })
        .run(conn)
        .await?;

        let stream = result.stream_and_drop::<Self::QueryOutput>().await?;

        Ok(stream)
    }
}

struct ImageList {
    filter: String,
}
