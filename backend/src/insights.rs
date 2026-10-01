use crate::{Server, auth::authenticate, db::Database, error::Result, model::*};
use axum::{Json, extract::State, http::HeaderMap};
use chrono::{DateTime, Datelike, Utc};
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub fn summarize(
    conn: &rusqlite::Connection,
    household: &str,
    state: &HouseholdState,
) -> Result<Value> {
    let events=conn.prepare("SELECT at,detail FROM events WHERE household_id=?1 AND kind='checkItem' AND undone=0 ORDER BY at LIMIT 10000")?.query_map(params![household],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
    let mut purchases: BTreeMap<String, Vec<(DateTime<Utc>, Value)>> = BTreeMap::new();
    let mut weekdays = BTreeMap::<String, u32>::new();
    let mut baskets = BTreeMap::<String, Vec<String>>::new();
    for (at, detail) in events {
        let detail: Value = serde_json::from_str(&detail)?;
        let Some(id) = detail["productId"].as_str() else {
            continue;
        };
        let Ok(at) = DateTime::parse_from_rfc3339(&at) else {
            continue;
        };
        let at = at.with_timezone(&Utc);
        let history = purchases.entry(id.to_string()).or_default();
        if detail["checked"] == false {
            history.pop();
            continue;
        }
        if detail["checked"] != true {
            continue;
        }
        history.push((at, detail));
    }
    let mut restock = vec![];
    let mut habits = vec![];
    let zone = state
        .preferences
        .time_zone
        .parse::<chrono_tz::Tz>()
        .unwrap_or(chrono_tz::UTC);
    for (id, history) in &purchases {
        let Some((last, detail)) = history.last() else {
            continue;
        };
        let product = crate::domain::product(state, id).ok();
        let Some(product) = product else { continue };
        let days = history
            .iter()
            .map(|(at, _)| at.with_timezone(&zone).date_naive())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut intervals = days
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).num_days())
            .filter(|days| *days >= 1 && *days <= 180)
            .collect::<Vec<_>>();
        intervals.sort();
        let interval = if intervals.len() >= 2 {
            Some(intervals[intervals.len() / 2])
        } else {
            None
        };
        let elapsed = (Utc::now() - *last).num_days();
        let note = history
            .iter()
            .filter_map(|(_, detail)| detail["note"].as_str())
            .filter(|note| !note.is_empty())
            .fold(BTreeMap::<String, u32>::new(), |mut counts, note| {
                *counts.entry(note.into()).or_default() += 1;
                counts
            })
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(note, _)| note)
            .unwrap_or_default();
        let store = history
            .iter()
            .filter_map(|(_, detail)| detail["storeId"].as_str())
            .fold(BTreeMap::<String, u32>::new(), |mut counts, store| {
                *counts.entry(store.into()).or_default() += 1;
                counts
            })
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(store, _)| store)
            .unwrap_or_default();
        let package = history
            .iter()
            .fold(
                BTreeMap::<String, (usize, &Value)>::new(),
                |mut counts, (_, detail)| {
                    if let (Some(quantity), Some(unit)) =
                        (detail["quantity"].as_f64(), detail["unit"].as_str())
                    {
                        let amount = crate::domain::normalize(&Amount {
                            quantity,
                            unit: unit.into(),
                        });
                        let key = serde_json::to_string(&amount).expect("finite purchase amount");
                        let entry = counts.entry(key).or_insert((0, detail));
                        entry.0 += 1;
                        entry.1 = detail;
                    }
                    counts
                },
            )
            .into_values()
            .max_by_key(|(count, _)| *count)
            .map(|(_, detail)| detail)
            .unwrap_or(detail);
        habits.push(json!({"productId":id,"name":product.name,"purchases":history.len(),"intervalDays":interval,"confidence":if intervals.len()>=5 {"established"}else if intervals.len()>=2 {"emerging"}else{"learning"},"lastBought":last,"preferredNote":note,"quantity":package["quantity"],"unit":package["unit"],"storeId":store}));
        for (at, _) in history {
            let day = at.with_timezone(&zone);
            *weekdays.entry(day.weekday().to_string()).or_default() += 1;
            baskets
                .entry(day.date_naive().to_string())
                .or_default()
                .push(id.clone());
        }
        let feedback = state.suggestion_feedback.get(&format!("restock:{id}"));
        let hidden = feedback.is_some_and(|feedback| {
            feedback.status == "dismissed" && feedback.at.as_str() > last.to_rfc3339().as_str()
                || feedback
                    .until
                    .as_ref()
                    .is_some_and(|until| until.as_str() > Utc::now().to_rfc3339().as_str())
        });
        if let Some(interval) = interval
            && elapsed >= interval
            && !hidden
            && !state.archived_product_ids.contains(id)
            && !state
                .items
                .iter()
                .any(|item| item.id == *id && !item.checked)
        {
            restock.push(json!({"product":product,"quantity":package["quantity"],"unit":package["unit"],"note":note,"intervalDays":interval,"daysSince":elapsed,"confidence":if intervals.len()>=5 {"established"}else{"emerging"},"reason":format!("Bought on {} separate days, usually about every {} days; last bought {} days ago. Worth checking.",days.len(),interval,elapsed)}));
        }
    }
    let mut pairs = BTreeMap::<(String, String), u32>::new();
    for ids in baskets.values() {
        let ids = ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        for (i, left) in ids.iter().enumerate() {
            for right in ids.iter().skip(i + 1) {
                *pairs
                    .entry(((*left).clone(), (*right).clone()))
                    .or_default() += 1;
            }
        }
    }
    let mut associations = pairs
        .into_iter()
        .filter(|(_, count)| *count >= 3)
        .collect::<Vec<_>>();
    associations.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    Ok(
        json!({"restock":restock,"habits":habits,"shoppingDays":weekdays,"associations":associations.into_iter().take(12).map(|((a,b),count)|json!({"products":[a,b],"daysTogether":count})).collect::<Vec<_>>(),"purchaseCount":purchases.values().map(Vec::len).sum::<usize>()}),
    )
}
pub async fn insights(State(server): State<Server>, headers: HeaderMap) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    let conn = server.db.lock()?;
    let state = Database::snapshot(&conn, &actor.household_id)?.state;
    Ok(Json(summarize(&conn, &actor.household_id, &state)?))
}
