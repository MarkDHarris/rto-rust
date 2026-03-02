use crate::data::persistence::Persistable;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    pub date: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct EventData {
    pub events: Vec<Event>,
}

impl Persistable for EventData {
    fn filename() -> &'static str {
        "events.json"
    }
    fn is_json() -> bool {
        true
    }
}

impl EventData {
    pub fn add(&mut self, event: Event) {
        self.events.push(event);
        self.events.sort_by(|a, b| a.date.cmp(&b.date));
    }

    pub fn remove(&mut self, date: &str, description: &str) {
        self.events
            .retain(|e| !(e.date == date && e.description == description));
    }

    #[allow(dead_code)]
    pub fn all(&self) -> Vec<Event> {
        self.events.clone()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn get_event_map(&self) -> HashMap<String, Vec<&Event>> {
        let mut map: HashMap<String, Vec<&Event>> = HashMap::new();
        for event in &self.events {
            map.entry(event.date.clone()).or_default().push(event);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(date: &str, desc: &str) -> Event {
        Event {
            date: date.to_string(),
            description: desc.to_string(),
        }
    }

    #[test]
    fn test_add_inserts_event() {
        let mut data = EventData::default();
        data.add(ev("2025-03-01", "Team lunch"));
        assert_eq!(data.len(), 1);
        assert_eq!(data.events[0].description, "Team lunch");
    }

    #[test]
    fn test_add_sorts_by_date() {
        let mut data = EventData::default();
        data.add(ev("2025-03-10", "Later event"));
        data.add(ev("2025-03-01", "Earlier event"));
        assert_eq!(data.events[0].date, "2025-03-01");
        assert_eq!(data.events[1].date, "2025-03-10");
    }

    #[test]
    fn test_remove_deletes_matching_event() {
        let mut data = EventData::default();
        data.add(ev("2025-03-01", "Meeting"));
        data.add(ev("2025-03-02", "Lunch"));
        data.remove("2025-03-01", "Meeting");
        assert_eq!(data.len(), 1);
        assert_eq!(data.events[0].description, "Lunch");
    }

    #[test]
    fn test_remove_requires_both_date_and_description() {
        let mut data = EventData::default();
        data.add(ev("2025-03-01", "Meeting"));
        data.remove("2025-03-01", "Wrong description");
        assert_eq!(data.len(), 1);
        data.remove("2025-12-31", "Meeting");
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_remove_nonexistent_is_noop() {
        let mut data = EventData::default();
        data.remove("2025-03-01", "Nothing");
        assert!(data.is_empty());
    }

    #[test]
    fn test_all_returns_copy() {
        let mut data = EventData::default();
        data.add(ev("2025-03-01", "Test"));
        let all = data.all();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_get_event_map_groups_by_date() {
        let mut data = EventData::default();
        data.add(ev("2025-03-01", "Event A"));
        data.add(ev("2025-03-01", "Event B"));
        data.add(ev("2025-03-02", "Event C"));
        let map = data.get_event_map();
        assert_eq!(map["2025-03-01"].len(), 2);
        assert_eq!(map["2025-03-02"].len(), 1);
    }

    #[test]
    fn test_get_event_map_empty() {
        let data = EventData::default();
        let map = data.get_event_map();
        assert!(map.is_empty());
    }

    #[test]
    fn test_default_event_data_is_empty() {
        let data = EventData::default();
        assert!(data.is_empty());
    }
}
