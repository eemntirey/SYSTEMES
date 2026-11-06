using System;
using Systemes.Native.Core;

namespace Systemes.Native
{
    class Program
    {
        static void Main(string[] args)
        {
            System.Console.OutputEncoding = System.Text.Encoding.UTF8;
            System.Console.InputEncoding = System.Text.Encoding.UTF8;
            NativeServer.Start();
        }
    }
}
