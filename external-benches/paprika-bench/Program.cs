using Paprika.Db;


Console.WriteLine("Hello, World!");

using var db = new NativeMemoryPagedDb(1024 * 1024UL);
var tx = db.Begin();

var key = new byte[32];

for (byte i = 0; i < byte.MaxValue; i++)
{
    key[0] = 0x12;
    key[1] = 0x34;
    key[2] = 0x56;
    key[3] = 0x78;
    key[31] = i;

    tx.Set(key, key);
}

Console.WriteLine($"Used memory {db.TotalUsedPages:P}");