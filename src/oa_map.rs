use std::cell::RefCell;

pub trait Hashable {
    fn hash(&self) -> u32;
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Entry<K: Hashable, V> {
    key: Option<K>,
    value: Option<V>,
    empty: bool,
    int_key: u32,
}

#[derive(Debug)]
pub struct OAMap<K: Hashable, V> {
    arr: Vec<RefCell<Entry<K, V>>>,
    entry_count: usize,
    capacity: usize,
    cap_ratio: usize
}

impl Hashable for String {
    fn hash(&self) -> u32 {
        let mut h: u32 = 0;
        for b in self.bytes() {
            h += u32::from(b);
        }
        return h;
    }
}

impl<K: Hashable, V> Entry<K, V> {

    fn new() -> Entry<K, V> {
        Entry {key: None, value: None, int_key: 0, empty: true}
    }

    fn populate(&mut self, key: K, value: V) {
        if let None = self.key {
            self.key = Some(key);
        }
        self.value = Some(value);
        if let Some(k) = &self.key {
            self.int_key = k.hash();
        }

        self.empty = false;
    }
}

impl<K: Hashable + Clone, V: Clone> OAMap<K, V> where K: PartialEq{
    pub fn new() -> OAMap<K, V> {
        let e = std::iter::repeat_with(|| RefCell::new(Entry::new()))
            .take(1000)
            .collect::<Vec<_>>();
        return OAMap { arr: e, entry_count: 0, capacity: 1000, cap_ratio: 0 };
    }


    pub fn new_with_capacity(capacity: usize) -> OAMap<K, V> {
        let e = std::iter::repeat_with(|| RefCell::new(Entry::new()))
            .take(capacity)
            .collect::<Vec<_>>();
        return OAMap { arr: e, entry_count: 0, capacity: capacity, cap_ratio: 0 };
    }

    pub fn find_address(&self, key: &K, arr_len: usize) -> usize {
        let hash = key.hash();
        let address = hash % u32::try_from(arr_len).unwrap();
        return address.try_into().unwrap();
    }

    pub fn get(&self, key: K) -> Option<V> {
        let mut address = self.find_address(&key, self.arr.len());
        let mut entry = self.arr.get(address).unwrap().borrow();
        while address < self.arr.len() {
            if let Some(k) = &entry.key {
                if *k == key && !entry.empty {
                    break;
                }
            }
            address += 1;
            if let Some(e) = self.arr.get(address) {
                entry = e.borrow();
            }
        }
        if let Some(k) = &entry.key {
            if *k == key && !entry.empty {
                return Some(entry.value.clone()?);
            }
        }
        return None;
    }

    pub fn contains_key(&self, key: K) -> bool {
        return self.get(key).is_some();
    }

    pub fn put(&mut self, key: K, value: V) {
        if self.cap_ratio > 50 {
            self.resize();
        }
        let mut address = self.find_address(&key, self.arr.len());
        let mut entry = self.arr.get(address).unwrap().borrow_mut();
        while address < self.arr.len() {
            if entry.empty {
                entry.populate(key, value);
                self.entry_count += 1;
                self.cap_ratio = 100 * self.entry_count / self.arr.len();
                return;
            }
            if let Some(k) = &entry.key {
                if *k == key {
                    entry.populate(key, value);
                    return;
                }
            }
            address += 1;
            entry = self.arr.get(address).unwrap().borrow_mut();
        }
        // TODO: There should be some resize logic here as well
    }

    pub fn delete(&mut self, key: K) {
        let mut address = self.find_address(&key, self.arr.len());
        let mut entry = self.arr.get(address).unwrap().borrow_mut();
        let okey = Some(key);
        while entry.key != okey && address < self.arr.len() {
            address += 1;
            entry = self.arr.get(address).unwrap().borrow_mut();
        }
        if entry.key == okey {
            entry.empty = true;
            self.entry_count -= 1;
            self.cap_ratio = 100 * self.entry_count / self.arr.len();
        }
    }

    fn resize(&mut self) {
        let mut map: OAMap<K, V> = OAMap::new_with_capacity(self.capacity * 2);
        for entry in self.arr.iter() {
            if !entry.borrow().empty {
                match &entry.borrow().key {
                    Some(k) => {
                        let val = self.get(k.clone());
                        map.put(k.clone(), val.unwrap());
                    },
                    None => {
                        // shouldn't happen
                    }
                }
            }
        }
        self.arr = map.arr;
        self.entry_count = map.entry_count;
        self.capacity = map.capacity;
        self.cap_ratio = map.cap_ratio;
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_put() {
        let mut map: OAMap<String, String> = OAMap::new();
        map.put("my_key".to_string(), "my_value".to_string());
        let out = map.get("my_key".to_string());
        assert_eq!(out.unwrap(), "my_value");
    }

    #[test]
    fn test_put_overwrite() {
        let mut map: OAMap<String, String> = OAMap::new();
        map.put("my_key".to_string(), "my_value".to_string());
        map.put("my_key".to_string(), "other_value".to_string());
        let out = map.get("my_key".to_string());
        assert_eq!(out.unwrap(), "other_value");
    }

    #[test]
    fn test_get_non_existant() {
        let map: OAMap<String, String> = OAMap::new();
        let out = map.get("some_key".to_string());
        assert!(out.is_none());
    }

    #[test]
    fn test_delete() {
        let mut map: OAMap<String, String> = OAMap::new();
        map.put("my_key".to_string(), "my_value".to_string());
        map.put("my_key".to_string(), "other_value".to_string());
        map.delete("my_key".to_string());
        let out = map.get("my_key".to_string());
        assert!(out.is_none());
    }


    #[test]
    fn test_put_trigger_resize() {
        let mut map: OAMap<String, String> = OAMap::new_with_capacity(2);
        let test_data = [("fist_key", "first_value"), ("second_key", "second_value"),("third_key", "third_value"),("fourth_key", "fourth_value"),("fifth_key", "fifth_value"),];
        for e in test_data.iter() {
            map.put(e.0.to_string(), e.1.to_string());
        }
        assert_eq!(map.capacity, 8);
        for e in test_data.iter() {
            let out = map.get(e.0.to_string());
            assert!(out.is_some());
            assert_eq!(out.unwrap(), e.1);
        }
    }
}
