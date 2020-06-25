use std::time::SystemTime;

use serenity::{
    builder::{CreateEmbed, CreateMessage},
    utils::Color,
};

use chrono::prelude::*;

pub enum GuestResponse {
    AlreadyOver(SystemTime),
    AlreadyGuest(SystemTime),
    AlreadyHasMember,
    InternalError,
    Sucess(SystemTime),
}

#[inline(always)]
fn embed_base(emb: &mut CreateEmbed) -> &mut CreateEmbed {
    emb.title("Guest").timestamp(&Utc::now())
}

#[inline(always)]
fn embed_failure(emb: &mut CreateEmbed) -> &mut CreateEmbed {
    embed_base(emb).colour(Color::RED)
}

#[inline(always)]
fn embed_success(emb: &mut CreateEmbed) -> &mut CreateEmbed {
    embed_base(emb).colour(Color::DARK_GREEN)
}

impl GuestResponse {
    pub fn present(x: SystemTime) -> Self {
        match x >= SystemTime::now() {
            true => Self::AlreadyOver(x),
            false => Self::AlreadyGuest(x)
        }
    }

    pub fn send<'a, 'b>(&self, msg: &'a mut CreateMessage<'b>) -> &'a mut CreateMessage<'b> {
        match self {
            Self::AlreadyOver(exp) => {
                msg.embed(|emb: &mut CreateEmbed| {
                    embed_failure(emb).description(format!(
                        "Error! Your guest role has expired on {}.",
                        DateTime::<Local>::from(*exp)
                    ))
                });
            }
            Self::AlreadyGuest(exp) => {
                msg.embed(|emb: &mut CreateEmbed| {
                    embed_failure(emb).description(format!(
                        "Error! You already have the guest role. It will expire on {}.",
                        DateTime::<Local>::from(*exp)
                    ))
                });
            }
            Self::InternalError => {
                msg.embed(|emb: &mut CreateEmbed| {
                    embed_failure(emb).description(
                        "An internal error occured while processing the request. Your guest role ha snot been added."
                    )
                });
            }
            Self::AlreadyHasMember => {
                msg.embed(|emb: &mut CreateEmbed| {
                    embed_failure(emb).description("Error! You are already a member.")
                });
            }
            Self::Sucess(exp) => {
                msg.embed(|emb: &mut CreateEmbed| {
                    embed_success(emb).description(format!(
                        "Success! You have been given the guest role. It will expire on {}.",
                        DateTime::<Local>::from(*exp)
                    ))
                });
            }
        };
        msg
    }
}