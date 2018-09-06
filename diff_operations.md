# Diff operations theory

This theory was used in implementation of diff combine functions.

The purpose of combined diff is an ability to rebuild target file with multiple diffs with no intermediate temp output.

### Summing Diffs

```
skip(x) + skip(y)       = skip(x + y)
skip(x) + add(y)        = skip(x) add(y)
skip(x) + remove(y)     = skip(x) remove(y)
skip(x) + replace(y, z) = skip(x) replace(y, z)

add(x) + skip(y)       = add(x) skip(y)
add(x) + add(y)        = add(x + y)
add(x) + remove(y)     = add(x) remove(y)
add(x) + replace(y, z) = add(x) replace(y, z)

remove(x) + skip(y)       = remove(x) skip(y)
remove(x) + add(y)        = replace(x, y)
remove(x) + remove(y)     = remove(x + y)
remove(x) + replace(y, z) = replace(x + y, z)

replace(x, y) + skip(z)       = replace(x, y) skip(z)
replace(x, y) + add(z)        = replace(x, y + z)
replace(x, y) + remove(z)     = replace(x, y) remove(z)
replace(x, y) + replace(z, w) = replace(x, y) replace(z, w)
```

### Combining Diffs

"|" symbol means transitive diff

```
skip(x) | skip(y) =
	x = y: skip(x)
	x > y: skip(y) next(skip(x - y))
	x < y: skip(x) next(nil, skip(y - x))

skip(x) | add(y) =
	x = y: add(y) next(skip(x))

skip(x) | remove(y) =
	x = y: remove(x)
	x > y: remove(y) next(skip(x - y))
	x < y: remove(x) next(nil , remove(y - x))

skip(x) | replace(y, z) =
	x = y : remove(x) next(nil, add(z))
	x > y : remove(y) next(skip(x - y), add(z))
	x < y : remove(x) next(nil, replace(y - x, z))

add(x) | skip(y) =
	x = y: add(x)
	x > y: add(y) next(add(y..x))
	x < y: add(x) next(nil , skip(y - x))

add(x) | add (y) = add(y) next(add(x))

add(x) | remove(y) =
	x = y: nil
	x > y: next(add(y..x))
	x < y: next(nil, remove(y - x))

add(x) | replace(y, z) =
	x = y : next(nil, add(z))
	x > y : add(z) next(add(y..x))
	x < y : next(nil, replace(y - x, z))

remove(x) | skip(y) = remove(x) next(nil, skip(y))

remove(x) | add(y) = remove(x) nextb(nil, add(y))

remove(x) | remove(y) = remove(x) nextb(nil, remove(y))

remove(x) | replace(y, z) = remove(x) nextb(nil, replace(y, z))

replace(x, y) | skip(z) = remove(x) next(add(y), skip(z))

replace(x, y) | add(z) = remove(x) next(add(y), add(z))

replace(x, y) | remove(z) = remove(x) next(add(y), remove(z))

replace(x, y) | replace(z, w) = remove(x) next(add(y), replace(z, w))
```
