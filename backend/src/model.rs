use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Clone, Debug, Default, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct Amount {
    pub quantity: f64,
    pub unit: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct Product {
    pub id: String,
    pub name: String,
    pub category: String,
    pub icon: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub interval: Option<f64>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct PlanCredit {
    pub day: String,
    pub product_id: String,
    pub quantity: f64,
    pub unit: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct ShoppingItem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub icon: String,
    pub quantity: f64,
    pub unit: String,
    pub note: String,
    pub checked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub interval: Option<f64>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plan_credits: Option<Vec<PlanCredit>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub checked_before: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub manual_amount: Option<Amount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub amounts: Option<Vec<Amount>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct Ingredient {
    pub product_id: String,
    pub quantity: f64,
    pub unit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub image: String,
    pub minutes: u32,
    pub tag: String,
    pub servings: f64,
    pub ingredients: Vec<Ingredient>,
    pub steps: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub favorite: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct Store {
    pub id: String,
    pub name: String,
    pub location: String,
    pub order: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub image: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct PantryItem {
    pub amounts: Vec<Amount>,
    pub confirmed: bool,
    pub observed_at: String,
    pub expires_at: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct MealFeedback {
    pub recipe_id: String,
    pub status: String,
    pub servings: f64,
    pub at: String,
    pub rating: Option<u8>,
    pub consumed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct SuggestionFeedback {
    pub status: String,
    pub until: Option<String>,
    pub at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct Preferences {
    pub time_zone: String,
    pub servings: f64,
    pub dietary: Vec<String>,
    pub avoid: Vec<String>,
    pub novelty: f64,
    pub max_weekday_minutes: u32,
    pub auto_restock: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            time_zone: "Europe/Helsinki".into(),
            servings: 4.0,
            dietary: vec![],
            avoid: vec![],
            novelty: 0.25,
            max_weekday_minutes: 35,
            auto_restock: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct HouseholdState {
    #[serde(default)]
    pub garments: Vec<crate::clothing::Garment>,
    #[serde(default)]
    pub children: Vec<crate::clothing::Child>,
    #[serde(default)]
    pub locations: Vec<crate::clothing::StorageLocation>,
    #[serde(default)]
    pub sale_listings: Vec<crate::clothing::SaleListing>,
    pub items: Vec<ShoppingItem>,
    pub stores: Vec<Store>,
    pub store_id: String,
    pub plan: BTreeMap<String, String>,
    pub purchases: BTreeMap<String, String>,
    pub custom_products: Vec<Product>,
    pub custom_recipes: Vec<Recipe>,
    pub favorite_recipe_ids: Vec<String>,
    pub plan_coverage: BTreeMap<String, BTreeMap<String, Vec<Amount>>>,
    #[serde(default)]
    pub purchased_coverage: BTreeMap<String, BTreeMap<String, Vec<Amount>>>,
    pub recipe_book: Vec<Recipe>,
    pub groups: Vec<String>,
    pub pantry: BTreeMap<String, PantryItem>,
    pub servings: BTreeMap<String, f64>,
    pub meal_feedback: BTreeMap<String, MealFeedback>,
    pub suggestion_feedback: BTreeMap<String, SuggestionFeedback>,
    pub preferences: Preferences,
    #[serde(default)]
    pub product_images: BTreeMap<String, String>,
    #[serde(default)]
    pub product_groups: BTreeMap<String, String>,
    #[serde(default)]
    pub product_details: BTreeMap<String, Product>,
    #[serde(default)]
    pub archived_product_ids: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct Member {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct SessionInfo {
    pub user: Member,
    pub household_id: String,
    pub household_name: String,
    pub members: Vec<Member>,
    pub ai_enabled: bool,
    pub image_enabled: bool,
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct Snapshot {
    pub revision: u32,
    pub state: HouseholdState,
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct Change {
    pub collection: String,
    pub key: String,
    pub field: Option<String>,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, export_to = "../../src/lib/api/")]
pub enum Operation {
    Patch {
        changes: Vec<Change>,
        label: String,
    },
    CheckItem {
        id: String,
        checked: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        expected: Option<Box<ShoppingItem>>,
    },
    AddItem {
        product_id: String,
        quantity: f64,
        unit: String,
        note: String,
        accumulate: bool,
    },
    AddIngredients {
        ingredients: Vec<Ingredient>,
        multiplier: f64,
    },
    RemoveItem {
        id: String,
    },
    EditItem {
        id: String,
        quantity: f64,
        unit: String,
        category: String,
        note: String,
        expected: Box<ShoppingItem>,
    },
    MealFeedback {
        day: String,
        recipe_id: String,
        status: String,
        consume: bool,
        rating: Option<u8>,
    },
    FinishTrip,
    ObservePantry {
        product_id: String,
        quantity: f64,
        unit: String,
        expires_at: Option<String>,
    },
    Undo {
        event_id: String,
    },
    ResetHistory,
    ApplyProposal {
        proposal_id: String,
        shopping_indices: Vec<usize>,
        recipe_ids: Vec<String>,
        days: Vec<String>,
    },
}

#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct Mutation {
    pub id: String,
    pub operation: Operation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub store_id: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct HistoryEvent {
    pub id: String,
    pub actor: String,
    pub kind: String,
    pub at: String,
    pub detail: serde_json::Value,
    pub undone: bool,
}

#[derive(Deserialize)]
pub struct Seed {
    pub catalog: Vec<Product>,
    pub recipes: Vec<Recipe>,
    pub categories: Vec<String>,
}

pub fn seed() -> Seed {
    serde_json::from_str(include_str!("../seed.json")).expect("valid compiled seed catalog")
}
pub fn fresh_state(samples: bool, time_zone: String) -> HouseholdState {
    let seed = seed();
    HouseholdState {
        garments: vec![],
        children: vec![],
        locations: vec![],
        sale_listings: vec![],
        items: vec![],
        stores: vec![Store {
            id: "our-store".into(),
            name: "Our shop".into(),
            image: None,
            location: "".into(),
            order: seed.categories.clone(),
        }],
        store_id: "our-store".into(),
        plan: BTreeMap::new(),
        purchases: BTreeMap::new(),
        custom_products: vec![],
        custom_recipes: vec![],
        favorite_recipe_ids: vec![],
        plan_coverage: BTreeMap::new(),
        purchased_coverage: BTreeMap::new(),
        recipe_book: if samples { seed.recipes } else { vec![] },
        groups: seed.categories,
        pantry: BTreeMap::new(),
        servings: BTreeMap::new(),
        meal_feedback: BTreeMap::new(),
        suggestion_feedback: BTreeMap::new(),
        preferences: Preferences {
            time_zone,
            ..Preferences::default()
        },
        product_images: BTreeMap::new(),
        product_groups: BTreeMap::new(),
        product_details: BTreeMap::new(),
        archived_product_ids: vec![],
    }
}
