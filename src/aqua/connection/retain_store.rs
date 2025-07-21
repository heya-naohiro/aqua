use std::sync::Arc;

use dashmap::DashMap;
use mqtt_coder::mqtt::Publish;
use once_cell::sync::Lazy;
use tracing::debug;

pub static RETAIN_STORE: Lazy<RetainStore> = Lazy::new(|| RetainStore::new());

#[derive(Clone)]
pub struct RetainStore {
    kvs: Arc<DashMap<String, Publish>>,
}

impl RetainStore {
    pub fn new() -> Self {
        RetainStore {
            kvs: Arc::new(DashMap::new()),
        }
    }

    pub fn update_retain(&self, topic: String, pkt: Publish) {
        debug!("update retain {:?}", topic);
        self.kvs.insert(topic.clone(), pkt);
    }
    pub fn remove_retain(&self, topic: &str) {
        debug!("remove retain {:?}", topic);
        self.kvs.remove(topic);
    }
    pub fn check_retain(&self, topic_filter: String) -> Vec<Publish> {
        let mut ret = vec![];
        for entry in self.kvs.iter() {
            let topic = entry.key().clone();
            if self.is_match(&topic_filter, &topic) {
                if let Some(pkt) = self.kvs.get(&topic) {
                    ret.push(pkt.clone());
                }
            }
        }
        return ret;
    }

    fn is_match(&self, topic_filter: &String, topic: &String) -> bool {
        let filter_segment: Vec<&str> = topic_filter.split('/').collect();
        let topic_segment: Vec<&str> = topic.split('/').collect();

        let mut filter_index = 0;
        let mut topic_index = 0;
        while filter_index < filter_segment.len() {
            match filter_segment[filter_index] {
                "#" => {
                    // should be last
                    return filter_index == filter_segment.len() - 1;
                }
                "+" => {
                    if topic_index >= topic_segment.len() {
                        return false;
                    }
                    filter_index += 1;
                    topic_index += 1;
                }
                filter
                    if topic_index < topic_segment.len()
                        && filter == topic_segment[topic_index] =>
                {
                    filter_index += 1;
                    topic_index += 1;
                }
                _ => {
                    return false;
                }
            }
        }
        // have been comsumed all segments
        topic_index == topic_segment.len()
    }
}
