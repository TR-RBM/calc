int square(int x) { return x * x; }
int sum(const int *a, int n) { int s = 0; for (int i = 0; i < n; i++) s += a[i]; return s; }
int count_negative(const int *a, int n) { int c = 0; for (int i = 0; i < n; i++) if (a[i] < 0) c++; return c; }
void fill(int *a, int m, int n) { for (int i = 0; i < m; i++) for (int j = 0; j < n; j++) a[i*n+j] = i + j; }
int ten(void) { int s = 0; for (int i = 0; i < 10; i++) s += i; return s; }
long factorial(int n) { long r = 1; for (int i = 2; i <= n; i++) r *= i; return r; }
long fib(int n) { long a = 0, b = 1; for (int i = 0; i < n; i++) { long t = a + b; a = b; b = t; } return a; }
long dot(const int *a, const int *b, int n) { long s = 0; for (int i = 0; i < n; i++) s += (long)a[i] * b[i]; return s; }
void reverse(int *a, int n) { for (int i = 0, j = n - 1; i < j; i++, j--) { int t = a[i]; a[i] = a[j]; a[j] = t; } }
int array_max(const int *a, int n) { int m = a[0]; for (int i = 1; i < n; i++) if (a[i] > m) m = a[i]; return m; }
void prefix(int *a, int n) { for (int i = 1; i < n; i++) a[i] += a[i-1]; }
int count_down(int n) { int s = 0; while (n-- > 0) s += n; return s; }
void matmul(const int *a, const int *b, int *c, int n) { for (int i = 0; i < n; i++) for (int j = 0; j < n; j++) { int s = 0; for (int k = 0; k < n; k++) s += a[i*n+k] * b[k*n+j]; c[i*n+j] = s; } }
int sum_step(const int *a, int n) { int s = 0; for (int i = 0; i < n; i += 3) s += a[i]; return s; }
int poly(const int *c, int n, int x) { int r = 0; for (int i = n - 1; i >= 0; i--) r = r * x + c[i]; return r; }
int calls(int n) { return square(n) + ten(); }
unsigned popcount(unsigned x) { unsigned c = 0; while (x) { c += x & 1; x >>= 1; } return c; }
int grade(int x) { switch (x) { case 0: return 5; case 1: return 7; case 2: return 11; case 3: return 13; case 4: return 17; case 5: return 19; default: return -1; } }
