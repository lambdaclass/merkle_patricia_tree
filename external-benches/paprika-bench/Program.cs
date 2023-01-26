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

public class BenchInsert
{
    private MemoryDb db;
    private PaprikaTree tree;
    private List<byte[]> keys;
    private List<byte[]> newKeys;
    private int index = 0;

    [Params(1000, 10_000, 100_000, 1_000_000)]
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

    [IterationSetup]
    public void IterSetup()
    {
        db = new MemoryDb(1024 * 1024 * 1024);
        tree = new PaprikaTree(db);
        foreach (var key in keys)
        {
            tree.Set(key, key);
        }
    }

    [Benchmark]
    public void Insert()
    {
        foreach (var key in newKeys)
        {
            tree.Set(key, key);
        }
    }
}
