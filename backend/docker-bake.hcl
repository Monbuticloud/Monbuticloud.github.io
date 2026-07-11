target "default" {
  context = "."
  tags = ["montheepic/homepage:latest"]
  output = ["type=docker"]
}

target "push" {
  context = "."
  tags = ["montheepic/homepage:latest"]
  output = ["type=image,push=true,compression=zstd,force-compression=true,oci-mediatypes=true,compression-level=22"]
}
