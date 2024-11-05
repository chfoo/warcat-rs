if (args.Length == 0)
{
    System.Console.WriteLine("Specify 'encode' or 'decode'");
    return 1;
}

if (args[0] == "encode")
{
    WarcatExample.Encode.Run();
}
else
{
    WarcatExample.Decode.Run();
}

return 0;
