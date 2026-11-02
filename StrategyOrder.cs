namespace ZapretGui;

// Compare number sequences as numbers: ALT2 precedes ALT10, without parsing overflow.
sealed class StrategyOrder : IComparer<string>
{
    public static readonly StrategyOrder Instance = new();

    public int Compare(string? left, string? right)
    {
        if (left is null || right is null) return StringComparer.OrdinalIgnoreCase.Compare(left, right);
        var i = 0; var j = 0;
        while (i < left.Length && j < right.Length)
        {
            if (char.IsAsciiDigit(left[i]) && char.IsAsciiDigit(right[j]))
            {
                var startLeft = i; var startRight = j;
                while (i < left.Length && char.IsAsciiDigit(left[i])) i++;
                while (j < right.Length && char.IsAsciiDigit(right[j])) j++;
                var digitsLeft = left.AsSpan(startLeft, i - startLeft).TrimStart('0');
                var digitsRight = right.AsSpan(startRight, j - startRight).TrimStart('0');
                var order = digitsLeft.Length.CompareTo(digitsRight.Length);
                if (order == 0) order = digitsLeft.SequenceCompareTo(digitsRight);
                if (order != 0) return order;
                order = (i - startLeft).CompareTo(j - startRight);
                if (order != 0) return order;
            }
            else
            {
                var order = char.ToUpperInvariant(left[i]).CompareTo(char.ToUpperInvariant(right[j]));
                if (order != 0) return order;
                i++; j++;
            }
        }
        return (left.Length - i).CompareTo(right.Length - j);
    }
}
