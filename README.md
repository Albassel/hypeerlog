# hypeerlog

 A **blazingly fast** HyperLogLog implementation in Rust that can be distributed across multiple devices
 
 This implementes all optimizations in the Google paper (except sparse, which is planned for later):  https://research.google.com/pubs/archive/40671.pdf
 
## Usage

### Estimating cardinality

```rust
use hypeerlog::Hypeerlog;

let elems = vec![1, 2, 3, 4, 5, 6, 7, 1, 1, 2];

let mut hll = Hypeerlog::new();
hll.insert_many(&elems);

// Should be within 2% of the real cardinality
hll.cardinality();
```

### Distributing the work

You can divide the dataset onto multiple computers, dump the hll when you finish adding the data, load the dump into another computer, merge all the hll, and then calculate the cardinality of the merged hll to get the cardinality for the whole dataset:


```rust
use hypeerlog::Hypeerlog;

let elems = vec![1, 2, 3, 4, 5, 6, 7, 1, 1, 2];

let mut hll_one = Hypeerlog::new();
hll_one.insert_many(&elems[0..5]);

let mut hll_two = Hypeerlog::new();
hll_two.insert_many(&elems[5..]);

hll_one.merge(hll_two).unwrap().cardinality();
```

# Contribution

Feel free to fork or do a pull request, but it is highly advised to read the Google paper before looking into the code in order to understand the internals

