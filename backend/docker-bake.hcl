# Run from backend/: bake resolves the context relative to THIS file, so the
# repo root is ".." and the Dockerfile is addressed from there.
target "default" {
  context    = ".."
  dockerfile = "backend/Dockerfile"
  tags       = ["montheepic/homepage:latest"]
  output     = ["type=docker"]
}

target "push" {
  context    = ".."
  dockerfile = "backend/Dockerfile"
  tags       = ["montheepic/homepage:latest"]
  output     = ["type=image,push=true,compression=zstd,force-compression=true,oci-mediatypes=true,compression-level=22"]
}
