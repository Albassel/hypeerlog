# hypeerlog

A blazingly fast HyperLogLog++ implementation designed for high-throughput, distributed cardinality estimation.

This crate faithfully implements the [Google HyperLogLog++ paper](https://research.google.com/pubs/archive/40671.pdf), including standard bias correction and linear counting for small cardinalities.

The HyperLogLog algorithm is a probabilistic data structure used to estimate the number of distinct elements in a set. It operates using a fixed amount of memory while keeping the relative estimation error exceptionally small.

## Features

- **Flexible Hashing**: Employs a custom, ultra-fast Murmur3 hasher by default (the gold standard for HyperLogLog sketches), while offering full generic support to drop in your own custom hasher implementation.
- **Configurable Accuracy**: Define your own precision or maximum relative error bounds to explicitly tune the exact accuracy vs. memory footprint trade-off required for your workload.
- **Production Ready**: Rigorously tested, fully micro-benchmarked, and meticulously optimized for raw performance.

> **Note on Design**: This crate intentionally omits the sparse register representation described in the paper. By focusing entirely on a flattened dense register footprint, it removes serialization layout overheads and yields cleaner optimization paths for distributed network/storage engines where fixed-size states are highly desirable.
 
## Usage

### Estimating cardinality

```rust
use hypeerlog::Hypeerlog;

let elems = vec![1, 2, 3, 4, 5, 6, 7, 1, 1, 2];

let mut hll = Hypeerlog::new();
hll.insert_many(&elems);

// Should be within 1% of the real cardinality
hll.cardinality();
```

```rust
use hypeerlog::Hypeerlog;

let elems = vec![1, 2, 3, 4, 5, 6, 7, 1, 1, 2];

let mut hll = Hypeerlog::new();
hll.insert_many(&elems);

// The estimation is guaranteed to be within the typical HLL error bounds (e.g., ~2%).
assert_eq!(hll.cardinality().floor(), 7.0); 
```


### Distributing the work

HyperLogLog sketches are perfectly additive. You can distribute massive datasets across multiple independent workers, compute highly efficient local sketches, and merge them later to find the global unique count.


```rust
use hypeerlog::Hypeerlog;

let elems = vec![1, 2, 3, 4, 5, 6, 7, 1, 1, 2];

let mut hll_one = Hypeerlog::new();
hll_one.insert_many(&elems[0..5]);

let mut hll_two = Hypeerlog::new();
hll_two.insert_many(&elems[5..]);

// Merge the second sketch into the first
let merged = hll_one.merge(hll_two).unwrap();

assert_eq!(merged.cardinality().floor(), 7.0);
```

## `no_std` Support

This crate features a highly constrained, lightweight memory profile, making it a perfect fit for resource-constrained or bare-metal environments. 

To enable standalone usage, activate the `no_std` feature in your `Cargo.toml`:

```toml
[dependencies]
hypeerlog = { version = "0.3.1", features = ["no_std"] }
```

All core estimation and merging features remain fully available in `no_std` mode via safe heap allocations handled contextually by the `alloc` crate.


# Contribution

Feel free to fork or do a pull request, but it is highly advised to read the Google paper before looking into the code in order to understand the internals

Contributions from the community are highly appreciated and can help improve this project. If you have any suggestions, feature requests, or bugs to report, please open an issue on GitHub. Additionally, if you want to contribute to the project, you can open a pull request with your proposed changes.

If you appreciate this project and would like to support its development, you can star the repository on GitHub or consider making a financial contribution. The project maintainer has set up a GitHub Sponsors page where you can make a recurring financial contribution to support the project's development. Any financial contribution, no matter how small, is greatly appreciated and helps ensure the continued development and maintenance of this project.

