// build: the compile → assemble → pack chain, on hvolt's own ArkTS toolchain.
//
// The tree is grouped by responsibility:
//   ets/       the ArkTS language layer (masking, preprocessing, build bodies)
//   lower/     turning a source file into the TypeScript the compiler accepts
//   compile/   the compiler-facing artefacts (abc, resources, source maps)
//   artifacts/ the files a package is assembled from
//   deps/      project dependencies and SDK facts

#[cfg(test)]
pub(crate) mod architecture;
pub(crate) mod artifacts;
pub(crate) mod compile;
pub(crate) mod deps;
pub(crate) mod ets;
pub(crate) mod ets_assemble;
pub(crate) mod ets_compile;
pub(crate) mod ets_transform;
pub(crate) mod lex;
pub(crate) mod lower;
pub(crate) mod stage;
