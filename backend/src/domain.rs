use crate::{
    error::{ApiError, Result},
    model::*,
};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn today(state: &HouseholdState) -> String {
    Utc::now()
        .with_timezone(
            &state
                .preferences
                .time_zone
                .parse::<chrono_tz::Tz>()
                .unwrap_or(chrono_tz::UTC),
        )
        .date_naive()
        .to_string()
}
pub fn normalize(amount: &Amount) -> Amount {
    let (unit, factor) = match amount.unit.trim().to_lowercase().as_str() {
        "g" | "gram" | "grams" => ("g".to_string(), 1.0),
        "kg" => ("g".into(), 1000.0),
        "ml" => ("ml".into(), 1.0),
        "l" | "litre" | "litres" | "liter" | "liters" => ("ml".into(), 1000.0),
        "pc" | "pcs" | "piece" | "pieces" => ("pc".into(), 1.0),
        "clove" | "cloves" => ("cloves".into(), 1.0),
        other => (other.to_string(), 1.0),
    };
    Amount {
        unit,
        quantity: (amount.quantity * factor * 10000.0).round() / 10000.0,
    }
}
pub fn totals(values: &[Amount]) -> Vec<Amount> {
    let mut map = BTreeMap::new();
    for amount in values {
        let amount = normalize(amount);
        *map.entry(amount.unit).or_insert(0.0) += amount.quantity;
    }
    map.into_iter()
        .filter(|(_, quantity)| *quantity > 0.00001)
        .map(|(unit, quantity)| Amount {
            unit,
            quantity: (quantity * 10000.0).round() / 10000.0,
        })
        .collect()
}
pub fn product(state: &HouseholdState, id: &str) -> Result<Product> {
    let product = seed()
        .catalog
        .into_iter()
        .chain(state.custom_products.iter().cloned())
        .find(|product| product.id == id)
        .ok_or_else(|| ApiError::bad("This product no longer exists."))?;
    Ok(customize_product(state, product))
}
fn customize_product(state: &HouseholdState, mut product: Product) -> Product {
    let id = product.id.clone();
    if let Some(details) = state.product_details.get(&id) {
        product = details.clone();
    }
    if let Some(image) = state.product_images.get(&id) {
        product.image = Some(image.clone());
    }
    if let Some(group) = state.product_groups.get(&id) {
        product.category = group.clone();
    }
    product
}
pub fn products(state: &HouseholdState, include_archived: bool) -> Vec<Product> {
    seed()
        .catalog
        .into_iter()
        .chain(state.custom_products.iter().cloned())
        .filter(|entry| include_archived || !state.archived_product_ids.contains(&entry.id))
        .map(|entry| customize_product(state, entry))
        .collect()
}
fn recipe(state: &HouseholdState, id: &str) -> Option<Recipe> {
    state
        .recipe_book
        .iter()
        .chain(state.custom_recipes.iter())
        .find(|recipe| recipe.id == id)
        .cloned()
}

pub fn outstanding(state: &HouseholdState, today: &str) -> Vec<PlanCredit> {
    let date = NaiveDate::parse_from_str(today, "%Y-%m-%d").expect("internal valid date");
    let end = (date - Duration::days(i64::from(date.weekday().num_days_from_monday()))
        + Duration::days(14))
    .to_string();
    let mut grouped: BTreeMap<(String, String, String), f64> = BTreeMap::new();
    for (day, id) in &state.plan {
        if day.as_str() < today
            || day >= &end
            || state
                .meal_feedback
                .get(day)
                .is_some_and(|feedback| matches!(feedback.status.as_str(), "cooked" | "skipped"))
        {
            continue;
        }
        let Some(recipe) = recipe(state, id) else {
            continue;
        };
        let factor = state
            .servings
            .get(day)
            .copied()
            .unwrap_or(state.preferences.servings)
            / recipe.servings;
        for ingredient in recipe.ingredients {
            let amount = normalize(&Amount {
                quantity: ingredient.quantity * factor,
                unit: ingredient.unit,
            });
            *grouped
                .entry((day.clone(), ingredient.product_id, amount.unit))
                .or_default() += amount.quantity;
        }
    }
    let mut remaining = Vec::new();
    for ((day, product_id, unit), quantity) in grouped {
        let covered = totals(
            state
                .plan_coverage
                .get(&day)
                .and_then(|day| day.get(&product_id))
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        )
        .iter()
        .find(|amount| amount.unit == unit)
        .map_or(0.0, |amount| amount.quantity);
        if quantity > covered + 0.00001 {
            remaining.push(PlanCredit {
                day,
                product_id,
                unit,
                quantity: quantity - covered,
            });
        }
    }
    // Confirmed stock and recent estimates cover requirements in chronological order.
    let mut stock: BTreeMap<(String, String), f64> = BTreeMap::new();
    for (id, pantry) in &state.pantry {
        if pantry
            .expires_at
            .as_ref()
            .is_some_and(|expiry| expiry.as_str() < today)
        {
            continue;
        }
        let fresh = pantry.confirmed
            || chrono::DateTime::parse_from_rfc3339(&pantry.observed_at)
                .is_ok_and(|at| Utc::now().signed_duration_since(at).num_days() < 7);
        if !fresh {
            continue;
        }
        for amount in totals(&pantry.amounts) {
            stock.insert((id.clone(), amount.unit), amount.quantity);
        }
    }
    for credit in &mut remaining {
        let stock = stock
            .entry((credit.product_id.clone(), credit.unit.clone()))
            .or_default();
        let covered = credit.quantity.min(*stock);
        *stock -= covered;
        credit.quantity -= covered;
    }
    remaining
        .into_iter()
        .filter(|credit| credit.quantity > 0.00001)
        .collect()
}

fn coverage(state: &mut HouseholdState, credits: &[PlanCredit], direction: f64) {
    for credit in credits {
        let amounts = state
            .plan_coverage
            .entry(credit.day.clone())
            .or_default()
            .entry(credit.product_id.clone())
            .or_default();
        let amount = normalize(&Amount {
            unit: credit.unit.clone(),
            quantity: credit.quantity,
        });
        let mut merged = totals(amounts);
        if let Some(previous) = merged
            .iter_mut()
            .find(|previous| previous.unit == amount.unit)
        {
            previous.quantity = (previous.quantity + direction * amount.quantity).max(0.0);
        } else if direction > 0.0 {
            merged.push(amount);
        }
        *amounts = totals(&merged);
    }
}

pub fn displayed(state: &HouseholdState, id: &str, today: &str) -> Option<ShoppingItem> {
    let stored = state.items.iter().find(|item| item.id == id);
    let credits = outstanding(state, today)
        .into_iter()
        .filter(|credit| credit.product_id == id)
        .collect::<Vec<_>>();
    let notes = credits
        .iter()
        .filter_map(|credit| state.plan.get(&credit.day).and_then(|id| recipe(state, id)))
        .flat_map(|recipe| recipe.ingredients)
        .filter(|ingredient| ingredient.product_id == id)
        .filter_map(|ingredient| ingredient.note)
        .filter(|note| !note.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(" · ");
    let needed = credits
        .into_iter()
        .filter(|credit| credit.product_id == id)
        .map(|credit| Amount {
            quantity: credit.quantity,
            unit: credit.unit,
        })
        .collect::<Vec<_>>();
    let mut combined = totals(&needed);
    if let Some(manual) =
        stored.filter(|item| !item.checked && item.origin.as_deref() != Some("meal-plan"))
    {
        let amount = normalize(&Amount {
            quantity: manual.quantity,
            unit: manual.unit.clone(),
        });
        if let Some(current) = combined
            .iter_mut()
            .find(|current| current.unit == amount.unit)
        {
            current.quantity = current.quantity.max(amount.quantity);
        } else {
            combined.push(amount.clone());
        }
        // Extra incompatible amounts are explicit quantities, not hidden in the description.
        for extra in manual.amounts.as_deref().unwrap_or(&[]) {
            let extra = normalize(extra);
            if extra.unit == amount.unit {
                continue;
            }
            if let Some(current) = combined
                .iter_mut()
                .find(|current| current.unit == extra.unit)
            {
                current.quantity = current.quantity.max(extra.quantity);
            } else {
                combined.push(extra);
            }
        }
    }
    if combined.is_empty() {
        return stored.filter(|item| item.checked).cloned();
    }
    let product = product(state, id).ok()?;
    let mut item = stored.cloned().unwrap_or(ShoppingItem {
        id: product.id,
        name: product.name,
        category: product.category,
        icon: product.icon,
        image: product.image,
        interval: product.interval,
        aliases: product.aliases,
        quantity: 0.0,
        unit: "pc".into(),
        note: String::new(),
        checked: false,
        origin: Some("meal-plan".into()),
        plan_credits: None,
        checked_before: None,
        manual_amount: None,
        amounts: None,
    });
    let original = normalize(&Amount {
        quantity: 1.0,
        unit: item.unit.clone(),
    });
    if let Some(matching) = combined
        .iter_mut()
        .find(|amount| amount.unit == original.unit)
    {
        matching.quantity /= original.quantity;
        matching.unit = item.unit.clone();
    }
    let primary = combined
        .iter()
        .find(|amount| amount.unit == item.unit)
        .unwrap_or(&combined[0]);
    if item.note.is_empty() {
        item.note = notes;
    }
    item.quantity = primary.quantity;
    item.unit = primary.unit.clone();
    item.checked = false;
    item.amounts = Some(combined);
    Some(item)
}

fn adjust_stock(state: &mut HouseholdState, id: &str, amounts: &[Amount], direction: f64) {
    let item = state.pantry.entry(id.into()).or_insert(PantryItem {
        amounts: vec![],
        confirmed: false,
        observed_at: Utc::now().to_rfc3339(),
        expires_at: None,
        note: String::new(),
    });
    let mut merged = totals(&item.amounts);
    for amount in totals(amounts) {
        if let Some(current) = merged
            .iter_mut()
            .find(|current| current.unit == amount.unit)
        {
            current.quantity = (current.quantity + direction * amount.quantity).max(0.0);
        } else if direction > 0.0 {
            merged.push(amount);
        }
    }
    item.amounts = totals(&merged);
    item.confirmed = false;
    item.observed_at = Utc::now().to_rfc3339();
}

pub fn apply(state: &mut HouseholdState, operation: &Operation, today: &str) -> Result<Value> {
    let detail = match operation {
        Operation::Patch { changes, label } => {
            if changes.len() > 500 {
                return Err(ApiError::bad("Too many changes in one operation."));
            }
            apply_changes(state, changes)?;
            reconcile_reservations(state);
            json!({"label":label})
        }
        Operation::CheckItem {
            id,
            checked,
            expected,
        } => {
            let Some(item) = displayed(state, id, today) else {
                return Err(ApiError::bad("This item is no longer on the list."));
            };
            if item.checked == *checked {
                return Ok(json!({"id":id,"unchanged":true}));
            }
            if let Some(expected) = expected
                && totals(&item.amounts.clone().unwrap_or_else(|| {
                    vec![Amount {
                        quantity: item.quantity,
                        unit: item.unit.clone(),
                    }]
                })) != totals(&expected.amounts.clone().unwrap_or_else(|| {
                    vec![Amount {
                        quantity: expected.quantity,
                        unit: expected.unit.clone(),
                    }]
                }))
            {
                return Err(ApiError::conflict());
            }
            let position = state.items.iter().position(|stored| stored.id == *id);
            let previous = position.map(|index| state.items[index].clone());
            if *checked {
                let credits = outstanding(state, today)
                    .into_iter()
                    .filter(|credit| credit.product_id == *id)
                    .collect::<Vec<_>>();
                coverage(state, &credits, 1.0);
                for credit in &credits {
                    let entry = state
                        .purchased_coverage
                        .entry(credit.day.clone())
                        .or_default()
                        .entry(credit.product_id.clone())
                        .or_default();
                    entry.push(Amount {
                        quantity: credit.quantity,
                        unit: credit.unit.clone(),
                    });
                    *entry = totals(entry);
                }
                // Purchased plan quantities are reserved by credits; add only surplus to pantry.
                let mut surplus = totals(item.amounts.as_deref().unwrap_or(&[Amount {
                    quantity: item.quantity,
                    unit: item.unit.clone(),
                }]));
                for required in totals(
                    &credits
                        .iter()
                        .map(|credit| Amount {
                            quantity: credit.quantity,
                            unit: credit.unit.clone(),
                        })
                        .collect::<Vec<_>>(),
                ) {
                    if let Some(amount) = surplus
                        .iter_mut()
                        .find(|amount| amount.unit == required.unit)
                    {
                        amount.quantity = (amount.quantity - required.quantity).max(0.0);
                    }
                }
                adjust_stock(state, id, &surplus, 1.0);
                let mut checked_item = item.clone();
                checked_item.checked = true;
                checked_item.plan_credits = Some(credits);
                checked_item.checked_before =
                    Some(previous.as_ref().is_some_and(|item| item.checked));
                checked_item.manual_amount = previous
                    .as_ref()
                    .filter(|item| !item.checked && item.origin.is_none())
                    .map(|item| Amount {
                        quantity: item.quantity,
                        unit: item.unit.clone(),
                    });
                if let Some(index) = position {
                    state.items[index] = checked_item;
                } else {
                    state.items.push(checked_item);
                }
                state.purchases.insert(id.clone(), Utc::now().to_rfc3339());
            } else {
                let previous =
                    previous.ok_or_else(|| ApiError::bad("This checked item no longer exists."))?;
                coverage(state, previous.plan_credits.as_deref().unwrap_or(&[]), -1.0);
                for credit in previous.plan_credits.as_deref().unwrap_or(&[]) {
                    if let Some(amounts) = state
                        .purchased_coverage
                        .get_mut(&credit.day)
                        .and_then(|day| day.get_mut(id))
                    {
                        for amount in amounts.iter_mut() {
                            if amount.unit == credit.unit {
                                amount.quantity = (amount.quantity - credit.quantity).max(0.0)
                            }
                        }
                        *amounts = totals(amounts);
                    }
                }
                // Restoring means this item was not bought. Remove its pantry
                // entry entirely, including surplus released by changed plans.
                state.pantry.remove(id);
                let has_demand = outstanding(state, today)
                    .iter()
                    .any(|credit| credit.product_id == *id);
                let restored = &mut state.items[position.expect("checked item exists")];
                restored.checked = has_demand && restored.checked_before.unwrap_or(false);
                if let Some(amount) = restored.manual_amount.take() {
                    restored.quantity = amount.quantity;
                    restored.unit = amount.unit;
                }
                if !has_demand {
                    restored.origin = None;
                    restored.checked = false;
                }
                restored.plan_credits = None;
                restored.checked_before = None;
                restored.amounts = None;
            }
            json!({"productId":id,"quantity":item.quantity,"unit":item.unit,"amounts":item.amounts,"note":item.note,"checked":checked,"storeId":state.store_id})
        }
        Operation::AddItem {
            product_id,
            quantity,
            unit,
            note,
            accumulate,
        } => {
            add_item(state, product_id, *quantity, unit, note, *accumulate)?;
            json!({"productId":product_id,"quantity":quantity,"unit":unit,"note":note})
        }
        Operation::AddIngredients {
            ingredients,
            multiplier,
        } => {
            if !multiplier.is_finite()
                || *multiplier <= 0.0
                || *multiplier > 100.0
                || ingredients.len() > 100
            {
                return Err(ApiError::bad("Invalid serving amount."));
            }
            for ingredient in ingredients {
                add_item(
                    state,
                    &ingredient.product_id,
                    ingredient.quantity * multiplier,
                    &ingredient.unit,
                    ingredient.note.as_deref().unwrap_or(""),
                    true,
                )?;
            }
            json!({"ingredients":ingredients,"multiplier":multiplier})
        }
        Operation::RemoveItem { id } => {
            let credits = outstanding(state, today)
                .into_iter()
                .filter(|credit| credit.product_id == *id)
                .collect::<Vec<_>>();
            coverage(state, &credits, 1.0);
            state.items.retain(|item| item.id != *id);
            json!({"productId":id})
        }
        Operation::EditItem {
            id,
            quantity,
            unit,
            category,
            note,
            expected,
        } => {
            let current = displayed(state, id, today)
                .ok_or_else(|| ApiError::bad("This item no longer exists."))?;
            // A concurrent check, amount edit, group edit or description edit needs review.
            if current.checked != expected.checked
                || current.quantity != expected.quantity
                || current.unit != expected.unit
                || current.note != expected.note
                || current.category != expected.category
            {
                return Err(ApiError::conflict());
            }
            let changed = current.quantity != *quantity || current.unit != *unit;
            if changed {
                let credits = outstanding(state, today)
                    .into_iter()
                    .filter(|credit| credit.product_id == *id)
                    .collect::<Vec<_>>();
                coverage(state, &credits, 1.0);
            }
            let position = state.items.iter().position(|item| item.id == *id);
            let mut item = position
                .map(|index| state.items[index].clone())
                .unwrap_or(current);
            item.note = note.clone();
            item.category = category.clone();
            state.product_groups.insert(id.clone(), category.clone());
            if changed {
                item.quantity = *quantity;
                item.unit = unit.clone();
                item.origin = None;
                item.amounts = None;
                item.checked = expected.checked;
            }
            if let Some(index) = position {
                state.items[index] = item;
            } else {
                state.items.push(item);
            }
            json!({"productId":id})
        }
        Operation::MealFeedback {
            day,
            recipe_id,
            status,
            consume,
            rating,
        } => {
            if state.plan.get(day) != Some(recipe_id) {
                return Err(ApiError::conflict());
            }
            if !matches!(status.as_str(), "cooked" | "skipped")
                || rating.is_some_and(|rating| !(1..=5).contains(&rating))
            {
                return Err(ApiError::bad("Invalid meal feedback."));
            }
            if state.meal_feedback.get(day).is_some_and(|feedback| {
                feedback.recipe_id == *recipe_id && feedback.status == *status
            }) {
                return Ok(json!({"unchanged":true}));
            }
            if state.meal_feedback.contains_key(day) {
                return Err(ApiError::bad(
                    "Undo the previous meal feedback before changing it.",
                ));
            }
            let recipe = recipe(state, recipe_id)
                .ok_or_else(|| ApiError::bad("This recipe no longer exists."))?;
            let servings = state
                .servings
                .get(day)
                .copied()
                .unwrap_or(state.preferences.servings);
            if *consume && status == "cooked" {
                for ingredient in &recipe.ingredients {
                    let required = ingredient.quantity * servings / recipe.servings;
                    let credit = state
                        .plan_coverage
                        .get(day)
                        .and_then(|day| day.get(&ingredient.product_id))
                        .map(|amounts| {
                            totals(amounts)
                                .iter()
                                .filter(|amount| {
                                    amount.unit
                                        == normalize(&Amount {
                                            quantity: required,
                                            unit: ingredient.unit.clone(),
                                        })
                                        .unit
                                })
                                .map(|amount| amount.quantity)
                                .sum::<f64>()
                        })
                        .unwrap_or(0.0);
                    let normalized = normalize(&Amount {
                        quantity: required,
                        unit: ingredient.unit.clone(),
                    });
                    adjust_stock(
                        state,
                        &ingredient.product_id,
                        &[Amount {
                            quantity: (normalized.quantity - credit).max(0.0),
                            unit: normalized.unit,
                        }],
                        -1.0,
                    );
                }
            }
            state.meal_feedback.insert(
                day.clone(),
                MealFeedback {
                    recipe_id: recipe_id.clone(),
                    status: status.clone(),
                    servings,
                    at: Utc::now().to_rfc3339(),
                    rating: *rating,
                    consumed: *consume && status == "cooked",
                },
            );
            reconcile_reservations(state);
            json!({"day":day,"recipeId":recipe_id,"status":status,"servings":servings,"rating":rating})
        }
        Operation::FinishTrip => {
            let count = state.items.iter().filter(|item| item.checked).count();
            state.items.retain(|item| !item.checked);
            json!({"count":count,"storeId":state.store_id})
        }
        Operation::ObservePantry {
            product_id,
            quantity,
            unit,
            expires_at,
        } => {
            product(state, product_id)?;
            if *quantity != 0.0 {
                valid_amount(*quantity, unit)?;
            } else if unit.trim().is_empty() || !text(unit, 30) {
                return Err(ApiError::bad("Choose a unit."));
            }
            let observed = normalize(&Amount {
                quantity: *quantity,
                unit: unit.clone(),
            });
            let mut available = observed.quantity;
            let mut removed = vec![];
            for (day, entries) in &mut state.purchased_coverage {
                if day.as_str() < today {
                    continue;
                }
                if let Some(amounts) = entries.get_mut(product_id) {
                    for amount in amounts
                        .iter_mut()
                        .filter(|amount| amount.unit == observed.unit)
                    {
                        let retained = available.min(amount.quantity);
                        available -= retained;
                        let excess = amount.quantity - retained;
                        if excess > 0.0 {
                            removed.push(PlanCredit {
                                day: day.clone(),
                                product_id: product_id.clone(),
                                unit: observed.unit.clone(),
                                quantity: excess,
                            });
                        }
                        amount.quantity = retained;
                    }
                    *amounts = totals(amounts);
                }
            }
            coverage(state, &removed, -1.0);
            state.pantry.insert(
                product_id.clone(),
                PantryItem {
                    amounts: if available > 0.0 {
                        vec![Amount {
                            quantity: available,
                            unit: observed.unit,
                        }]
                    } else {
                        vec![]
                    },
                    confirmed: true,
                    observed_at: Utc::now().to_rfc3339(),
                    expires_at: expires_at.clone(),
                    note: String::new(),
                },
            );
            json!({"productId":product_id,"quantity":quantity,"unit":unit,"confirmed":true})
        }
        Operation::Undo { .. } | Operation::ResetHistory | Operation::ApplyProposal { .. } => {
            return Err(ApiError::bad(
                "This operation is handled by the history service.",
            ));
        }
    };
    validate(state)?;
    Ok(detail)
}

fn add_item(
    state: &mut HouseholdState,
    id: &str,
    quantity: f64,
    unit: &str,
    note: &str,
    accumulate: bool,
) -> Result<()> {
    valid_amount(quantity, unit)?;
    let product = product(state, id)?;
    if let Some(item) = state.items.iter_mut().find(|item| item.id == id) {
        if item.checked {
            item.quantity = quantity;
            item.unit = unit.into();
            item.amounts = None;
            item.plan_credits = None;
            item.manual_amount = None;
        } else if accumulate {
            let amount = normalize(&Amount {
                quantity,
                unit: unit.into(),
            });
            let previous = normalize(&Amount {
                quantity: item.quantity,
                unit: item.unit.clone(),
            });
            if amount.unit == previous.unit {
                item.quantity += amount.quantity
                    / normalize(&Amount {
                        quantity: 1.0,
                        unit: item.unit.clone(),
                    })
                    .quantity;
            } else {
                let mut amounts = item.amounts.clone().unwrap_or_else(|| {
                    vec![Amount {
                        quantity: item.quantity,
                        unit: item.unit.clone(),
                    }]
                });
                amounts.push(amount);
                item.amounts = Some(totals(&amounts));
            }
        }
        item.checked = false;
        item.origin = None;
        if !note.trim().is_empty()
            && !item
                .note
                .split(" · ")
                .any(|existing| existing == note.trim())
        {
            item.note = if item.note.is_empty() {
                note.trim().into()
            } else {
                format!("{} · {}", item.note, note.trim())
            };
        }
    } else {
        state.items.push(ShoppingItem {
            id: product.id,
            name: product.name,
            category: product.category,
            icon: product.icon,
            image: product.image,
            interval: product.interval,
            aliases: product.aliases,
            quantity,
            unit: unit.trim().into(),
            note: note.trim().into(),
            checked: false,
            origin: None,
            plan_credits: None,
            checked_before: None,
            manual_amount: None,
            amounts: None,
        });
    }
    Ok(())
}

pub fn apply_changes(state: &mut HouseholdState, changes: &[Change]) -> Result<()> {
    let before = state.clone();
    let mut value = serde_json::to_value(&*state)?;
    for change in changes {
        patch(&mut value, change)?;
    }
    *state = serde_json::from_value(value)?;
    crate::clothing::invalidate_approvals(&before, state);
    validate(state)
}
pub fn apply_history_changes(state: &mut HouseholdState, changes: &[Change]) -> Result<()> {
    let mut value = serde_json::to_value(&*state)?;
    for change in changes {
        if change.collection == "purchases" {
            let map = value["purchases"].as_object_mut().expect("model map");
            let current = map.get(&change.key).cloned().unwrap_or(Value::Null);
            if !json_equal(&current, &change.before) {
                return Err(ApiError::conflict());
            }
            if change.after.is_null() {
                map.remove(&change.key);
            } else {
                map.insert(change.key.clone(), change.after.clone());
            }
        } else if change.collection == "storeId" {
            replace(&mut value["storeId"], &change.before, &change.after)?;
        } else {
            patch(&mut value, change)?;
        }
    }
    *state = serde_json::from_value(value)?;
    validate(state)
}
fn patch(state: &mut Value, change: &Change) -> Result<()> {
    let lists = [
        "garments",
        "children",
        "locations",
        "saleListings",
        "items",
        "stores",
        "customProducts",
        "customRecipes",
        "recipeBook",
    ];
    let maps = [
        "plan",
        "planCoverage",
        "purchasedCoverage",
        "pantry",
        "servings",
        "mealFeedback",
        "suggestionFeedback",
        "productImages",
        "productGroups",
        "productDetails",
    ];
    let scalars = [
        "groups",
        "favoriteRecipeIds",
        "preferences",
        "archivedProductIds",
    ];
    if !lists.contains(&change.collection.as_str())
        && !maps.contains(&change.collection.as_str())
        && !scalars.contains(&change.collection.as_str())
    {
        return Err(ApiError::bad("This field cannot be edited directly."));
    }
    if change.key.len() > 100 || change.field.as_ref().is_some_and(|field| field.len() > 100) {
        return Err(ApiError::bad("Invalid change key."));
    }
    let collection = &mut state[&change.collection];
    if lists.contains(&change.collection.as_str()) {
        let list = collection
            .as_array_mut()
            .ok_or_else(|| ApiError::bad("Invalid collection."))?;
        let position = list
            .iter()
            .position(|entry| entry["id"].as_str() == Some(&change.key));
        if let Some(field) = &change.field {
            let entry = position
                .map(|index| &mut list[index])
                .ok_or_else(ApiError::conflict)?;
            replace(&mut entry[field], &change.before, &change.after)?;
        } else {
            let current = position
                .map(|index| list[index].clone())
                .unwrap_or(Value::Null);
            if json_equal(&current, &change.after) {
                return Ok(());
            }
            if !json_equal(&current, &change.before) {
                return Err(ApiError::conflict());
            }
            if change.after.is_null() {
                if let Some(index) = position {
                    list.remove(index);
                }
            } else {
                if change.after["id"].as_str() != Some(&change.key) {
                    return Err(ApiError::bad("Entry IDs cannot change."));
                }
                if let Some(index) = position {
                    list[index] = change.after.clone();
                } else {
                    list.push(change.after.clone());
                }
            }
        }
    } else if maps.contains(&change.collection.as_str()) {
        if change.field.is_some() {
            return Err(ApiError::bad("Map fields are edited as entries."));
        }
        let map = collection
            .as_object_mut()
            .ok_or_else(|| ApiError::bad("Invalid map."))?;
        let current = map.get(&change.key).cloned().unwrap_or(Value::Null);
        if json_equal(&current, &change.after) {
            return Ok(());
        }
        if !json_equal(&current, &change.before) {
            return Err(ApiError::conflict());
        }
        if change.after.is_null() {
            map.remove(&change.key);
        } else {
            map.insert(change.key.clone(), change.after.clone());
        }
    } else {
        if !change.key.is_empty() {
            return Err(ApiError::bad("Invalid setting key."));
        }
        if let Some(field) = &change.field {
            if change.collection != "preferences" {
                return Err(ApiError::bad("Invalid setting field."));
            }
            replace(&mut collection[field], &change.before, &change.after)?;
        } else {
            replace(collection, &change.before, &change.after)?;
        }
    }
    Ok(())
}
// JavaScript serializes whole f64 quantities as integers. Compare their numeric
// values recursively without rounding distinct large integers through f64.
pub fn json_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) if a.is_f64() != b.is_f64() => {
            let (float, integer) = if a.is_f64() {
                (a.as_f64().unwrap(), b)
            } else {
                (b.as_f64().unwrap(), a)
            };
            if float.fract() != 0.0 {
                return false;
            }
            if let Some(value) = integer.as_i64() {
                float >= i64::MIN as f64 && float < i64::MAX as f64 && float as i64 == value
            } else if let Some(value) = integer.as_u64() {
                float >= 0.0 && float < u64::MAX as f64 && float as u64 == value
            } else {
                false
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| json_equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, a)| b.get(key).is_some_and(|b| json_equal(a, b)))
        }
        _ => a == b,
    }
}

fn replace(current: &mut Value, before: &Value, after: &Value) -> Result<()> {
    if json_equal(current, after) {
        return Ok(());
    }
    if !json_equal(current, before) {
        return Err(ApiError::conflict());
    }
    *current = after.clone();
    Ok(())
}

pub fn changes(before: &HouseholdState, after: &HouseholdState) -> Result<Vec<Change>> {
    let a = serde_json::to_value(before)?;
    let b = serde_json::to_value(after)?;
    let mut result = vec![];
    for collection in [
        "garments",
        "children",
        "locations",
        "saleListings",
        "items",
        "stores",
        "customProducts",
        "customRecipes",
        "recipeBook",
    ] {
        let old = a[collection].as_array().expect("model array");
        let new = b[collection].as_array().expect("model array");
        let keys = old
            .iter()
            .chain(new)
            .filter_map(|item| item["id"].as_str())
            .collect::<BTreeSet<_>>();
        for key in keys {
            let before = old
                .iter()
                .find(|item| item["id"].as_str() == Some(key))
                .cloned()
                .unwrap_or(Value::Null);
            let after = new
                .iter()
                .find(|item| item["id"].as_str() == Some(key))
                .cloned()
                .unwrap_or(Value::Null);
            if !json_equal(&before, &after) {
                result.push(Change {
                    collection: collection.into(),
                    key: key.into(),
                    field: None,
                    before,
                    after,
                });
            }
        }
    }
    for collection in [
        "plan",
        "planCoverage",
        "purchasedCoverage",
        "pantry",
        "servings",
        "mealFeedback",
        "suggestionFeedback",
        "productImages",
        "productGroups",
        "productDetails",
        "purchases",
    ] {
        let old = a[collection].as_object().expect("model map");
        let new = b[collection].as_object().expect("model map");
        let keys = old.keys().chain(new.keys()).collect::<BTreeSet<_>>();
        for key in keys {
            let before = old.get(key).cloned().unwrap_or(Value::Null);
            let after = new.get(key).cloned().unwrap_or(Value::Null);
            if !json_equal(&before, &after) {
                result.push(Change {
                    collection: collection.into(),
                    key: key.into(),
                    field: None,
                    before,
                    after,
                });
            }
        }
    }
    for collection in [
        "groups",
        "favoriteRecipeIds",
        "preferences",
        "storeId",
        "archivedProductIds",
    ] {
        if a[collection] != b[collection] {
            result.push(Change {
                collection: collection.into(),
                key: String::new(),
                field: None,
                before: a[collection].clone(),
                after: b[collection].clone(),
            });
        }
    }
    Ok(result)
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && !matches!(id, "__proto__" | "constructor" | "prototype")
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}
fn text(text: &str, max: usize) -> bool {
    text.chars().count() <= max && !text.chars().any(|character| character == '\0')
}
fn valid_amount(quantity: f64, unit: &str) -> Result<()> {
    if !quantity.is_finite()
        || quantity <= 0.0
        || quantity > 1_000_000.0
        || unit.trim().is_empty()
        || !text(unit, 30)
    {
        return Err(ApiError::bad("Choose a positive quantity and a unit."));
    }
    Ok(())
}
pub fn valid_image(image: &str) -> bool {
    image.is_empty()
        || image.starts_with("/images/") && !image.contains("..")
        || image.strip_prefix("/api/images/").is_some_and(valid_id)
}
pub fn validate(state: &HouseholdState) -> Result<()> {
    crate::clothing::validate(state)?;
    if serde_json::to_vec(state)?.len() > 4 * 1024 * 1024
        || state.items.len() > 2000
        || state.recipe_book.len() + state.custom_recipes.len() > 2000
        || state.custom_products.len() > 4000
    {
        return Err(ApiError::bad("The household has exceeded its data limit."));
    }
    if state.groups.is_empty()
        || state.groups.len() > 100
        || state
            .groups
            .iter()
            .any(|group| group.trim().is_empty() || !text(group, 60))
        || state.groups.iter().collect::<BTreeSet<_>>().len() != state.groups.len()
    {
        return Err(ApiError::bad(
            "Groups must have unique names, up to 60 characters.",
        ));
    }
    let groups = state.groups.iter().collect::<BTreeSet<_>>();
    let products = seed()
        .catalog
        .iter()
        .map(|product| product.id.clone())
        .chain(
            state
                .custom_products
                .iter()
                .map(|product| product.id.clone()),
        )
        .collect::<BTreeSet<_>>();
    let built_in_ids: BTreeSet<_> = seed().catalog.into_iter().map(|p| p.id).collect();
    if state
        .archived_product_ids
        .iter()
        .any(|id| !products.contains(id))
        || state
            .archived_product_ids
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != state.archived_product_ids.len()
        || state
            .product_details
            .iter()
            .any(|(id, p)| id != &p.id || !built_in_ids.contains(id))
        || state
            .custom_products
            .iter()
            .any(|p| built_in_ids.contains(&p.id))
    {
        return Err(ApiError::bad("Invalid catalog customization."));
    }
    for product in state
        .custom_products
        .iter()
        .chain(state.product_details.values())
    {
        if !valid_id(&product.id)
            || product.name.trim().is_empty()
            || !text(&product.name, 100)
            || !groups.contains(&product.category)
            || !text(&product.icon, 20)
            || product
                .image
                .as_ref()
                .is_some_and(|image| !valid_image(image))
            || product.aliases.len() > 30
            || product.aliases.iter().any(|alias| !text(alias, 80))
        {
            return Err(ApiError::bad("Invalid product details."));
        }
    }
    for (id, image) in &state.product_images {
        if !products.contains(id) || !valid_image(image) {
            return Err(ApiError::bad("Invalid product picture."));
        }
    }
    for (id, group) in &state.product_groups {
        if !products.contains(id) || !groups.contains(group) {
            return Err(ApiError::bad("Invalid product group."));
        }
    }
    if state.stores.is_empty() || state.stores.len() > 500 {
        return Err(ApiError::bad("Configure between 1 and 500 shops."));
    }
    for store in &state.stores {
        if !valid_id(&store.id)
            || store.name.trim().is_empty()
            || !text(&store.name, 80)
            || !text(&store.location, 120)
            || store
                .image
                .as_ref()
                .is_some_and(|image| !valid_image(image))
            || store.order.iter().collect::<BTreeSet<_>>() != groups
            || store.order.len() != groups.len()
        {
            return Err(ApiError::bad(
                "Each shop must contain every group exactly once.",
            ));
        }
    }
    if !state.stores.iter().any(|store| store.id == state.store_id) {
        state
            .stores
            .first()
            .ok_or_else(|| ApiError::bad("Choose a shop."))?;
    }
    for item in &state.items {
        valid_amount(item.quantity, &item.unit)?;
        if !products.contains(&item.id)
            || !text(&item.note, 1000)
            || !groups.contains(&item.category)
            || item.image.as_ref().is_some_and(|image| !valid_image(image))
        {
            return Err(ApiError::bad("Invalid shopping item."));
        }
        for amount in item.amounts.as_deref().unwrap_or(&[]) {
            valid_amount(amount.quantity, &amount.unit)?;
        }
    }
    let mut recipe_ids = BTreeSet::new();
    for recipe in state.recipe_book.iter().chain(&state.custom_recipes) {
        if !valid_id(&recipe.id)
            || !recipe_ids.insert(&recipe.id)
            || recipe.name.trim().is_empty()
            || !text(&recipe.name, 120)
            || !text(&recipe.subtitle, 8000)
            || recipe.minutes == 0
            || recipe.minutes > 1440
            || !recipe.servings.is_finite()
            || recipe.servings <= 0.0
            || recipe.servings > 100.0
            || !valid_image(&recipe.image)
            || recipe.ingredients.is_empty()
            || recipe.ingredients.len() > 100
            || recipe.steps.len() > 100
            || recipe.steps.is_empty()
            || !text(&recipe.tag, 60)
            || recipe.steps.iter().any(|step| !text(step, 3000))
            || recipe.tags.len() > 20
            || recipe.tags.iter().any(|tag| !text(tag, 60))
        {
            return Err(ApiError::bad("Invalid recipe details."));
        }
        for ingredient in &recipe.ingredients {
            valid_amount(ingredient.quantity, &ingredient.unit)?;
            if !products.contains(&ingredient.product_id)
                || ingredient
                    .note
                    .as_ref()
                    .is_some_and(|note| !text(note, 500))
            {
                return Err(ApiError::bad("A recipe ingredient no longer exists."));
            }
        }
    }
    for (day, id) in &state.plan {
        if NaiveDate::parse_from_str(day, "%Y-%m-%d").is_err() || !recipe_ids.contains(id) {
            return Err(ApiError::bad("Invalid meal plan entry."));
        }
    }
    for servings in state.servings.values() {
        if !servings.is_finite() || *servings <= 0.0 || *servings > 100.0 {
            return Err(ApiError::bad("Choose between 1 and 100 servings."));
        }
    }
    for collection in [&state.plan_coverage, &state.purchased_coverage] {
        for (day, entries) in collection {
            if NaiveDate::parse_from_str(day, "%Y-%m-%d").is_err() {
                return Err(ApiError::bad("Invalid ingredient coverage date."));
            }
            for (id, amounts) in entries {
                if !products.contains(id) || amounts.len() > 20 {
                    return Err(ApiError::bad("Invalid ingredient coverage."));
                }
                for amount in amounts {
                    valid_amount(amount.quantity, &amount.unit)?;
                }
            }
        }
    }
    for (day, feedback) in &state.meal_feedback {
        if NaiveDate::parse_from_str(day, "%Y-%m-%d").is_err()
            || !valid_id(&feedback.recipe_id)
            || !matches!(feedback.status.as_str(), "cooked" | "skipped")
            || !feedback.servings.is_finite()
            || feedback.servings <= 0.0
            || feedback.servings > 100.0
            || feedback
                .rating
                .is_some_and(|rating| !(1..=5).contains(&rating))
            || chrono::DateTime::parse_from_rfc3339(&feedback.at).is_err()
        {
            return Err(ApiError::bad("Invalid cooking feedback."));
        }
    }
    for feedback in state.suggestion_feedback.values() {
        if !matches!(
            feedback.status.as_str(),
            "accepted" | "dismissed" | "snoozed" | "edited"
        ) || chrono::DateTime::parse_from_rfc3339(&feedback.at).is_err()
            || feedback
                .until
                .as_ref()
                .is_some_and(|until| chrono::DateTime::parse_from_rfc3339(until).is_err())
        {
            return Err(ApiError::bad("Invalid suggestion feedback."));
        }
    }
    for (id, pantry) in &state.pantry {
        if !products.contains(id)
            || pantry.amounts.len() > 20
            || chrono::DateTime::parse_from_rfc3339(&pantry.observed_at).is_err()
            || pantry
                .expires_at
                .as_ref()
                .is_some_and(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err())
            || !text(&pantry.note, 500)
        {
            return Err(ApiError::bad("Invalid pantry observation."));
        }
        for amount in &pantry.amounts {
            valid_amount(amount.quantity, &amount.unit)?;
        }
    }
    let preferences = &state.preferences;
    if preferences.time_zone.parse::<chrono_tz::Tz>().is_err()
        || preferences.servings <= 0.0
        || preferences.servings > 100.0
        || preferences.novelty < 0.0
        || preferences.novelty > 1.0
        || preferences.max_weekday_minutes > 1440
        || preferences.dietary.len() > 30
        || preferences.avoid.len() > 100
        || preferences
            .dietary
            .iter()
            .chain(&preferences.avoid)
            .any(|value| !text(value, 100))
    {
        return Err(ApiError::bad("Invalid household preferences."));
    }
    // Duplicate IDs would make edits ambiguous.
    for ids in [
        state
            .items
            .iter()
            .map(|entry| &entry.id)
            .collect::<Vec<_>>(),
        state.stores.iter().map(|entry| &entry.id).collect(),
        state
            .custom_products
            .iter()
            .map(|entry| &entry.id)
            .collect(),
    ] {
        if ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err(ApiError::bad("Entries must have unique IDs."));
        }
    }
    Ok(())
}

// Keep purchased ingredients useful when plans change, without counting them twice.
fn reconcile_reservations(state: &mut HouseholdState) {
    let reserved = state.purchased_coverage.clone();
    for (day, products) in reserved {
        let feedback = state.meal_feedback.get(&day).map(|f| f.status.clone());
        if feedback.as_deref() == Some("cooked") {
            state.purchased_coverage.remove(&day);
            continue;
        }
        let recipe = state.plan.get(&day).and_then(|id| recipe(state, id));
        for (id, amounts) in products {
            let requirements = if feedback.as_deref() == Some("skipped") {
                vec![]
            } else {
                recipe
                    .as_ref()
                    .map(|recipe| {
                        totals(
                            &recipe
                                .ingredients
                                .iter()
                                .filter(|i| i.product_id == id)
                                .map(|i| Amount {
                                    quantity: i.quantity
                                        * state
                                            .servings
                                            .get(&day)
                                            .copied()
                                            .unwrap_or(state.preferences.servings)
                                        / recipe.servings,
                                    unit: i.unit.clone(),
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .unwrap_or_default()
            };
            let mut retained = vec![];
            let mut released = vec![];
            for amount in totals(&amounts) {
                let needed = requirements
                    .iter()
                    .find(|r| r.unit == amount.unit)
                    .map_or(0.0, |r| r.quantity);
                let keep = amount.quantity.min(needed);
                if keep > 0.0 {
                    retained.push(Amount {
                        quantity: keep,
                        unit: amount.unit.clone(),
                    })
                }
                if amount.quantity > keep {
                    released.push(Amount {
                        quantity: amount.quantity - keep,
                        unit: amount.unit,
                    })
                }
            }
            if !released.is_empty() {
                adjust_stock(state, &id, &released, 1.0);
                let credits = released
                    .iter()
                    .map(|a| PlanCredit {
                        day: day.clone(),
                        product_id: id.clone(),
                        quantity: a.quantity,
                        unit: a.unit.clone(),
                    })
                    .collect::<Vec<_>>();
                coverage(state, &credits, -1.0);
                for item in state.items.iter_mut().filter(|item| item.id == id) {
                    if let Some(credits) = &mut item.plan_credits {
                        for release in &released {
                            let mut remaining = release.quantity;
                            for credit in credits
                                .iter_mut()
                                .filter(|c| c.day == day && c.unit == release.unit)
                            {
                                let remove = credit.quantity.min(remaining);
                                credit.quantity -= remove;
                                remaining -= remove;
                            }
                        }
                        credits.retain(|c| c.quantity > 0.00001);
                    }
                }
            }
            state
                .purchased_coverage
                .entry(day.clone())
                .or_default()
                .insert(id, retained);
        }
    }
}

pub fn recipe_allowed(state: &HouseholdState, recipe: &Recipe) -> bool {
    let products = recipe
        .ingredients
        .iter()
        .filter_map(|i| product(state, &i.product_id).ok())
        .collect::<Vec<_>>();
    let dietary = state
        .preferences
        .dietary
        .iter()
        .map(|d| d.to_lowercase())
        .collect::<BTreeSet<_>>();
    if state.preferences.avoid.iter().any(|term| {
        products.iter().any(|p| {
            p.name.to_lowercase().contains(&term.to_lowercase())
                || p.aliases
                    .iter()
                    .any(|a| a.to_lowercase().contains(&term.to_lowercase()))
        })
    }) {
        return false;
    }
    if (dietary.contains("vegetarian") || dietary.contains("vegan"))
        && products
            .iter()
            .any(|p| matches!(p.category.as_str(), "Meat" | "Fish"))
    {
        return false;
    }
    if dietary.contains("vegan")
        && products
            .iter()
            .any(|p| matches!(p.category.as_str(), "Milk & eggs" | "Cheese") || p.id == "honey")
    {
        return false;
    }
    if dietary.contains("gluten-free")
        && products.iter().any(|p| {
            [
                "pasta", "bread", "flour", "bulgur", "couscous", "barley", "tortilla",
            ]
            .iter()
            .any(|word| p.name.to_lowercase().contains(word))
                && !p.name.to_lowercase().contains("gluten-free")
        })
    {
        return false;
    }
    true
}
