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

# GPU parity (#125): the stranger docs corpus and grid under RBQN_GPU=force vs RBQN_GPU=off,
# same binary; the two rbqn-side reports must be identical. Not in CI (no GPU runner); run on the bench.
# Needs BQN_SRC and CBQN_BIN as for `stranger` (the CBQN leg is only the scripts' reference).
GPU_PARITY_DIR ?= target/gpu-parity
.PHONY: gpu-parity
gpu-parity:
	cargo build --release
	mkdir -p $(GPU_PARITY_DIR)
	RBQN_GPU=off python3 -I bench/stranger/docs_diff.py > $(GPU_PARITY_DIR)/docs.off.txt
	RBQN_GPU=force python3 -I bench/stranger/docs_diff.py > $(GPU_PARITY_DIR)/docs.force.txt
	RBQN_GPU=off python3 -I bench/stranger/grid.py > $(GPU_PARITY_DIR)/grid.off.txt 2>/dev/null
	RBQN_GPU=force python3 -I bench/stranger/grid.py > $(GPU_PARITY_DIR)/grid.force.txt 2>/dev/null
	@d=0; for k in docs grid; do \
	  if diff -u $(GPU_PARITY_DIR)/$$k.off.txt $(GPU_PARITY_DIR)/$$k.force.txt > $(GPU_PARITY_DIR)/$$k.diff; then :; else d=$$((d+1)); cat $(GPU_PARITY_DIR)/$$k.diff; fi; done; \
	  echo "gpu-parity: docs [$$(head -1 $(GPU_PARITY_DIR)/docs.force.txt)] grid [$$(grep -m1 '^exprs=' $(GPU_PARITY_DIR)/grid.force.txt)] differing reports: $$d"; \
	  test $$d -eq 0
