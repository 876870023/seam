use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;

use super::{Live, Node};
use crate::error::{Result, SeamError};
use crate::util::hash2header;
use crate::{
    common::{CLIENT, USER_AGENT},
    util::parse_url,
};

const INIT_URL: &str = "https://api.live.bilibili.com/room/v1/Room/room_init";
const INFO_URL: &str =
    "https://api.live.bilibili.com/xlive/web-room/v1/index/getInfoByRoom?room_id=";
const PLAY_URL: &str = "https://api.live.bilibili.com/xlive/web-room/v2/index/getRoomPlayInfo";

/// bilibili直播
///
/// https://live.bilibili.com/
pub struct Client;

#[async_trait]
impl Live for Client {
    async fn get(&self, rid: &str, headers: Option<HashMap<String, String>>) -> Result<Node> {
        let resp = CLIENT
            .get(INIT_URL)
            .query(&[("id", rid)])
            .headers(hash2header(headers))
            .send()
            .await?
            .json::<Value>()
            .await?;

        // 获取真实房间号
        let rid = match resp["data"]["live_status"].as_i64() {
            Some(1) => resp["data"]["room_id"]
                .as_u64()
                .ok_or(SeamError::NeedFix("room_id"))?
                .to_string(),
            _ => return Err(SeamError::None),
        };

        let mut stream_info = get_bili_stream_info(&rid, 10000).await?;

        let max = stream_info
            .as_array()
            .ok_or(SeamError::NeedFix("stream_info"))?
            .iter()
            .map(|data| {
                data["format"][0]["codec"][0]["accept_qn"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| item.as_u64().unwrap())
                    .max()
                    .unwrap()
            })
            .max()
            .ok_or(SeamError::NeedFix("max"))?;

        if max != 10000 {
            stream_info = get_bili_stream_info(&rid, max).await?;
        }

        let mut urls = vec![];
        for obj in stream_info.as_array().ok_or(SeamError::NeedFix("obj"))? {
            for format in obj["format"]
                .as_array()
                .ok_or(SeamError::NeedFix("format"))?
            {
                for codec in format["codec"]
                    .as_array()
                    .ok_or(SeamError::NeedFix("codec"))?
                {
                    let base_url = codec["base_url"]
                        .as_str()
                        .ok_or(SeamError::NeedFix("base_url"))?;
                    for url_info in codec["url_info"]
                        .as_array()
                        .ok_or(SeamError::NeedFix("url_info"))?
                    {
                        let host = url_info["host"]
                            .as_str()
                            .ok_or(SeamError::NeedFix("host"))?;
                        let extra = url_info["extra"]
                            .as_str()
                            .ok_or(SeamError::NeedFix("extra"))?;
                        urls.push(parse_url(format!("{host}{base_url}{extra}")));
                    }
                }
            }
        }

        let json = CLIENT
            .get(format!("{}{}", INFO_URL, rid))
            .send()
            .await?
            .json::<Value>()
            .await?;

        let title = json["data"]["room_info"]["title"]
            .as_str()
            .unwrap_or("获取失败")
            .to_owned();

        let cover = json["data"]["room_info"]["cover"]
            .as_str()
            .unwrap_or("")
            .to_owned();

        let anchor = json["data"]["anchor_info"]["base_info"]["uname"]
            .as_str()
            .unwrap_or("")
            .to_owned();

        let head = json["data"]["anchor_info"]["base_info"]["face"]
            .as_str()
            .unwrap_or("")
            .to_owned();

        Ok(Node {
            rid,
            title,
            cover,
            anchor,
            head,
            urls,
        })
    }
}

/// 通过真实房间号获取直播源信息
/// 不带 cookie 只给 480P, 带 cookie 才给原画画质
pub async fn get_bili_stream_info(rid: &str, qn: u64, headers: Option<HashMap<String, String>>) -> Result<serde_json::Value> {
    let mut headers = hash2header(headers);
    headers.append("User-Agent", HeaderValue::from_static(USER_AGENT));
    Ok(CLIENT
        .get(PLAY_URL)
        .headers(headers)
        .query(&[
            ("room_id", rid),
            ("protocol", "0,1"),
            ("format", "0,1,2"),
            ("codec", "0,1"),
            ("qn", qn.to_string().as_str()),
            ("platform", "h5"),
            ("ptype", "8"),
        ])
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?["data"]["playurl_info"]["playurl"]["stream"]
        .to_owned())
}

#[cfg(test)]
macros::gen_test!(6);
