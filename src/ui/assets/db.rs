use std::borrow::Cow;

use anyhow::anyhow;
use smol::block_on;
use url::Url;

use crate::db::TursoDatabase;

pub fn load(pool: &TursoDatabase, url: Url) -> gpui::Result<Option<Cow<'static, [u8]>>> {
    match url.host_str().ok_or(anyhow!("missing table name"))? {
        "album" => {
            let mut segments = url.path_segments().ok_or(anyhow!("missing path"))?;
            let id = segments
                .next()
                .ok_or(anyhow!("missing id"))?
                .parse::<i64>()?;

            let image_type = segments.next().ok_or(anyhow!("missing image type"))?;

            let conn = pool.connect()?;
            let image = match image_type {
                "thumb" => block_on(async {
                    conn.query_one(
                        include_str!("../../../queries/assets/find_album_thumb.sql"),
                        (id,),
                        |row| Ok(row.get::<Vec<u8>>(0)?)
                    ).await
                })?,
                "full" => block_on(async {
                    conn.query_one(
                        include_str!("../../../queries/assets/find_album_art.sql"),
                        (id,),
                        |row| Ok(row.get::<Vec<u8>>(0)?)
                    ).await
                })?,
                _ => unimplemented!(),
            };

            Ok(Some(Cow::Owned(image)))
        }
        _ => Ok(None),
    }
}
