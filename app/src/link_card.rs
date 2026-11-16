//! The link card (#3080): ⇧ alone on a link, with its plate showing, shows what the page behind it
//! says about itself, its title, description and picture, in a card over the page. The core fetches
//! the page as no user would (bc_tab_link_card); what's read from it is `page`'s.

pub mod page;
pub mod picture;
