use std::time::SystemTime;

use serenity::{
    utils::Color,
    builder::{CreateMessage, CreateEmbed}
};

use chrono::prelude::*;

pub enum GuestResponse {
    AlreadyOver(SystemTime),
    AlreadyGuest(SystemTime),
    AlreadyHasMember,
    ErrorReadingDatabase,
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
    pub fn send<'a, 'b>(&self, msg: &'a mut CreateMessage<'b>) -> &'a mut CreateMessage<'b> {
        match self {
            Self::AlreadyOver(exp) => {
                msg.embed(|emb: &mut CreateEmbed| {
                    embed_failure(emb).description(
                        format!("Error! Your guest role has expired on {}.", DateTime::<Utc>::from(*exp))
                    )
                });
            },
            Self::AlreadyGuest(exp) => {
                msg.embed(|emb: &mut CreateEmbed| {
                    embed_failure(emb).description(
                        format!("Error! You already have the guest role. It will expire on {}.", DateTime::<Utc>::from(*exp))
                    )
                });
            },
            Self::ErrorReadingDatabase => {
                msg.embed(|emb: &mut CreateEmbed| {
                    embed_failure(emb).description(
                        "An error occured while reading the database. Your guest role has not been added."
                    )
                });
            },
            Self::AlreadyHasMember => {
                msg.embed(|emb: &mut CreateEmbed| {
                    embed_failure(emb).description(
                        "Error! You are already a member."
                    )
                });
            },
            Self::Sucess(exp) => {
                msg.embed(|emb: &mut CreateEmbed| {
                    embed_success(emb).description(
                        format!("Success! You have been given the guest role. It will expire on {}.", DateTime::<Utc>::from(*exp))
                    )
                }); 
            },
        };
        msg
    }
}

impl From<(SystemTime, bool)> for GuestResponse {
    fn from(x: (SystemTime, bool)) -> Self {
        match x.1 {
            true => Self::AlreadyOver(x.0),
            false => Self::AlreadyGuest(x.0)
        }
    }
}