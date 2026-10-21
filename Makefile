# No GPU CI runner exists; run the GPU correctness corpus locally (#13).
# CBQN_BIN=/path/to/BQN adds the CBQN leg.
.PHONY: gpu-corpus
gpu-corpus:
	cargo build --release
	./test_gpu_corpus.sh

# Stranger test (#71): diff rbqn against CBQN on the BQN docs and a primitive grid.
# Needs BQN_SRC=/path/to/BQN (docs checkout) and CBQN_BIN=/path/to/BQN. Not in CI.
.PHONY: stranger
stranger:
	cargo build --release
	python3 -I bench/stranger/docs_diff.py
	python3 -I bench/stranger/grid.py
