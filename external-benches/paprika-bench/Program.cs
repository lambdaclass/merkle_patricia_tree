using Paprika.Db;
using BenchmarkDotNet.Attributes;
using BenchmarkDotNet.Running;
using Tree;

var summary = BenchmarkRunner.Run<Bench>();

public class Bench
{
    private MemoryDb db;
    private PaprikaTree tree;
    private List<byte[]> keys;
    private int index = 0;

    [Params(1000, 10_000, 100_000, 1_000_000)]
    public int N;

    public Bench()
    {
        db = new MemoryDb(1024 * 1024 * 1024);
        tree = new PaprikaTree(db);
        keys = new List<byte[]>(N);
    }

    [GlobalSetup]
    public void Setup()
    {
        index = 0;
        Random rnd = new Random();
        for (int i = 0; i < N; i++)
        {
            var key = new byte[32];
            rnd.NextBytes(key);
            tree.Set(key, key);
            keys.Add(key);
        }
        Console.Out.WriteLine($"Keys len: {keys.Count}");
    }

    [Benchmark]
    public void Get()
    {
        var key = keys[index];
        index += 1;
        index = index % keys.Count;
        tree.TryGet(key, out var value);
    }
}
