using BenchmarkDotNet.Attributes;
using BenchmarkDotNet.Running;
using Tree;
using BenchmarkDotNet.Engines;

var summary = BenchmarkRunner.Run<Bench>();
var summary2 = BenchmarkRunner.Run<BenchInsert>();

public class Bench
{
    private MemoryDb db;
    private PaprikaTree tree;
    private List<byte[]> keys;
    private int index = 0;

    [Params(1000, 10_000, 100_000, 1_000_000, 10_000_000)]
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

[SimpleJob(RunStrategy.Monitoring, launchCount: 3, warmupCount: 0, iterationCount: 1000)]
public class BenchInsert
{
    private MemoryDb db;
    private PaprikaTree tree;
    private List<byte[]> keys;
    private List<byte[]> newKeys;
    private int index = 0;

    [Params(1000, 10_000, 100_000, 1_000_000, 10_000_000)]
    public int N;

    public BenchInsert()
    {
        db = new MemoryDb(1024 * 1024 * 1024);
        tree = new PaprikaTree(db);
        keys = new List<byte[]>(N);
        newKeys = new List<byte[]>(1000);
    }

    [GlobalSetup]
    public void Setup()
    {
        keys = new List<byte[]>(N);
        index = 0;
        Random rnd = new Random();
        for (int i = 0; i < N; i++)
        {
            var key = new byte[32];
            rnd.NextBytes(key);
            keys.Add(key);
            tree.Set(key, key);
        }

        while (newKeys.Count < 1000)
        {
            var key = new byte[32];
            rnd.NextBytes(key);
            if (!keys.Contains(key))
            {
                newKeys.Add(key);
            }
        }
    }

    [Benchmark]
    public void Insert()
    {
        var key = newKeys[index++];
        tree.Set(key, key);
    }
}
