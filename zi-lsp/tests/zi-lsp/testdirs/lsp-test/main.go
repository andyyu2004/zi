// An example go module to test lsp interactions
package main

func main() {
	f(42)
	g(42)
}

func f(i int) {
	g(i)
}
