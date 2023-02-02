package main

import (
	"math/rand"
	"testing"

	"github.com/ethereum/go-ethereum/core/rawdb"
	"github.com/ethereum/go-ethereum/trie"
	"golang.org/x/exp/constraints"
)

func main() {
}

func BenchmarkGet1k(b *testing.B)   { benchGet(b, 1000) }
func BenchmarkGet10k(b *testing.B)  { benchGet(b, 10000) }
func BenchmarkGet100k(b *testing.B) { benchGet(b, 100000) }
func BenchmarkGet1m(b *testing.B)   { benchGet(b, 1000000) }
func BenchmarkGet10m(b *testing.B)  { benchGet(b, 10000000) }

func BenchmarkInsert1k(b *testing.B)   { benchInsert(b, 1000) }
func BenchmarkInsert10k(b *testing.B)  { benchInsert(b, 10000) }
func BenchmarkInsert100k(b *testing.B) { benchInsert(b, 100000) }
func BenchmarkInsert1m(b *testing.B)   { benchInsert(b, 1000000) }
func BenchmarkInsert10m(b *testing.B)  { benchInsert(b, 10000000) }

func BenchmarkHash100(b *testing.B) { benchHash(b, 100) }
func BenchmarkHash500(b *testing.B) { benchHash(b, 500) }
func BenchmarkHash1k(b *testing.B)  { benchHash(b, 1000) }
func BenchmarkHash2k(b *testing.B)  { benchHash(b, 2000) }
func BenchmarkHash5k(b *testing.B)  { benchHash(b, 5000) }
func BenchmarkHash10k(b *testing.B) { benchHash(b, 10000) }

func benchGet(b *testing.B, benchElemCount int) {
	triedb := trie.NewDatabase(rawdb.NewMemoryDatabase())
	t := trie.NewEmpty(triedb)

	paths := make([][]byte, 0, benchElemCount)

	for i := 0; i < benchElemCount; i++ {
		path_len := 16 + rand.Intn(48)
		k := make([]byte, path_len)
		rand.Read(k)
		value := make([]byte, 32, 32)
		for i := 0; i < len(value); i++ {
			value[i] = 0
		}
		t.Update(k, value)
		paths = append(paths, k)
	}

	b.SetParallelism(1)
	b.ResetTimer()
	b.ReportAllocs()
	j := 0
	for i := 0; i < b.N; i++ {
		t.Get(paths[j])
		j = j + 1
		j = j % benchElemCount
	}
	b.StopTimer()
}

func min[T constraints.Ordered](a, b T) T {
	if a < b {
		return a
	}
	return b
}

func benchInsert(b *testing.B, benchElemCount int) {
	triedb := trie.NewDatabase(rawdb.NewMemoryDatabase())
	t := trie.NewEmpty(triedb)

	paths := make([][]byte, 0, benchElemCount)

	value := make([]byte, 32, 32)
	for i := 0; i < len(value); i++ {
		value[i] = 0
	}

	for i := 0; i < benchElemCount; i++ {
		path_len := 16 + rand.Intn(48)
		k := make([]byte, path_len)
		rand.Read(k)
		t.Update(k, value)
		paths = append(paths, k)
	}

	new_paths := make([][]byte, 0, 1000)

	for len(new_paths) < 1000 {
		path_len := 16 + rand.Intn(48)
		k := make([]byte, path_len)
		rand.Read(k)
		_, err := t.TryGet(k)

		if err == nil {
			new_paths = append(new_paths, k)
		}
	}

	const step = 1024

	b.SetParallelism(1)
	b.ReportAllocs()
	b.ResetTimer()
	b.StopTimer()

	for i := 0; i < b.N; i += step {

		tt := t.Copy()

		b.StartTimer()
		c := 0
		for j := i; j < min(b.N, i+step); j++ {
			tt.Update(new_paths[c], value)
			c = c + 1
			c = c % len(new_paths)
		}
		b.StopTimer()
	}
}

func benchHash(b *testing.B, benchElemCount int) {
	triedb := trie.NewDatabase(rawdb.NewMemoryDatabase())
	t := trie.NewEmpty(triedb)

	paths := make([][]byte, 0, benchElemCount)

	for i := 0; i < benchElemCount; i++ {
		path_len := 16 + rand.Intn(48)
		k := make([]byte, path_len)
		rand.Read(k)
		value := make([]byte, 32, 32)
		for i := 0; i < len(value); i++ {
			value[i] = 0
		}
		t.Update(k, value)
		paths = append(paths, k)
	}

	b.SetParallelism(1)
	b.ResetTimer()
	b.StopTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		tt := t.Copy()
		b.StartTimer()
		tt.Hash()
		b.StopTimer()
	}
	//b.StopTimer()
}
