use super::{sticker::Sticker, tg_get, Config, ImportConfig};
use crate::{database::Database, matrix};
use anyhow::anyhow;
use derive_getters::Getters;
use futures_util::future::join_all;
use serde::Deserialize;

#[cfg(feature = "log")]
use log::{info, warn};

#[derive(Clone, Debug, Deserialize, Getters, Hash)]
#[non_exhaustive]
pub struct StickerPack {
	pub(crate) name: String,
	pub(crate) title: String,
	pub(crate) is_animated: bool,
	pub(crate) is_video: bool,
	pub(crate) stickers: Vec<Sticker>
}

impl StickerPack {
	/// Request a stickerpack by its name.
	pub async fn get(name: &str, tg_config: &Config) -> anyhow::Result<Self> {
		let mut pack: Result<Self, anyhow::Error> = tg_get(tg_config, "getStickerSet", [("name", name)]).await;
		if let Ok(ref mut pack) = pack {
			for (i, sticker) in pack.stickers.iter_mut().enumerate() {
				sticker.pack_name = pack.name.clone();
				sticker.positon = i;
			}
		}
		pack
	}

	/// Import this pack to matrix.
	///
	/// This function can partially fail, where some sticker cannot be imported.
	/// Because of this, the postion of failed stickers and the error, which has occurred,
	/// will is also returned.
	///
	/// It should be checked if the stickerpack is empty.
	pub async fn import<'a, D>(
		self,
		tg_config: &Config,
		matrix_config: &matrix::Config,
		advance_config: &ImportConfig<'a, D>
	) -> (matrix::stickerpack::StickerPack, Vec<(usize, anyhow::Error)>)
	where
		D: Database
	{
		#[cfg(feature = "log")]
		info!("import Telegram stickerpack {}({})", self.title, self.name);
		#[cfg(feature = "log")]
		if self.is_video {
			warn!(
				"sticker pack {} includes video stickers. Import of video stickers is not supported and will be skipped.",
				self.name
			);
		}

		let stickers_import_futures = self
			.stickers
			.iter()
			.map(|f| f.import(tg_config, matrix_config, advance_config));
		let stickers = join_all(stickers_import_futures).await;

		let mut ok_stickers = Vec::new();
		let mut err_stickers = Vec::new();
		for (i, sticker) in stickers.into_iter().enumerate() {
			match sticker {
				Ok(value) => ok_stickers.push(value),
				Err(err) => err_stickers.push((i, err))
			}
		}

		let stickerpack = matrix::stickerpack::StickerPack {
			title: self.title.clone(),
			id: format!("tg_name_{}", self.name),
			tg_pack: Some((&self).into()),
			stickers: ok_stickers
		};
		#[cfg(feature = "log")]
		if stickerpack.stickers.is_empty() {
			warn!("imported pack {} is empty", self.name);
		}
		(stickerpack, err_stickers)
	}
}

/// Convert telegram stickerpack url to pack name.
///
/// The url must start with `https://t.me/addstickers/`, `t.me/addstickers/` or
/// `tg://addstickers?set=`.
pub fn pack_url_to_name(url: &str) -> anyhow::Result<&str> {
	url.strip_prefix("https://t.me/addstickers/").or_else(|| {
		url.strip_prefix("t.me/addstickers/")
	}).or_else(|| {
		url.strip_prefix("tg://addstickers?set=")
	}).ok_or_else(|| {
		anyhow!("{url:?} does not look like a Telegram StickerPack\nPack url should start with \"https://t.me/addstickers/\", \"t.me/addstickers/\" or \"tg://addstickers?set=\"")
	})
}
