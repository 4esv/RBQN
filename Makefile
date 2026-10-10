# No GPU CI runner exists; run the GPU correctness corpus locally (#13).
# CBQN_BIN=/path/to/BQN adds the CBQN leg.
.PHONY: gpu-corpus
gpu-corpus:
	cargo build --release
	./test_gpu_corpus.sh
