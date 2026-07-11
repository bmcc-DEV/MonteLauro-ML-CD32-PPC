/*
 * CDG² — string.h stubs (memset, memcpy)
 * Necessarios para o compilador quando -nostdlib.
 */

void *memset(void *s, int c, unsigned long n)
{
    unsigned char *p = s;
    for (unsigned long i = 0; i < n; i++) p[i] = (unsigned char)c;
    return s;
}

void *memcpy(void *dst, const void *src, unsigned long n)
{
    unsigned char *d = dst;
    const unsigned char *s = src;
    for (unsigned long i = 0; i < n; i++) d[i] = s[i];
    return dst;
}
