using System.Text;
namespace NotifyRedirect.Infrastructure;
public static class MessageSplitter
{
    public static IReadOnlyList<string> Split(string text,int maxUtf8Bytes)
    {
        if(maxUtf8Bytes<4)throw new ArgumentException("分段容量至少为 4 字节");
        var parts=new List<string>();var builder=new StringBuilder();int bytes=0;
        foreach(var rune in text.EnumerateRunes())
        {
            if(bytes+rune.Utf8SequenceLength>maxUtf8Bytes){parts.Add(builder.ToString());builder.Clear();bytes=0;}
            builder.Append(rune);bytes+=rune.Utf8SequenceLength;
        }
        if(builder.Length>0)parts.Add(builder.ToString());
        return parts.Count==0?[""]:parts;
    }
    public static string Limit(string text,int maxUtf8Bytes)
    {
        if(Encoding.UTF8.GetByteCount(text)<=maxUtf8Bytes)return text;
        const string suffix="\n[正文过长，已截断]";
        return Split(text,maxUtf8Bytes-Encoding.UTF8.GetByteCount(suffix))[0]+suffix;
    }
}
