// Copyright (c) 2023 Murilo Ijanc' <mbsd@m0x.ru>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//
// TODO: tag, auth, push, multi arch
//
use aws_config::meta::region::RegionProviderChain;
use aws_config::Region;
use bollard::image::{BuildImageOptions, BuilderVersion};
use bollard::models::BuildInfoAux;
use bollard::Docker;
use dockerfile_parser::Dockerfile;
use std::str::FromStr;

use futures_util::stream::StreamExt;

use base64::prelude::*;
use std::io::Write;

async fn get_credential() -> (String, String) {
    // Struct credentials to push
    // https://docs.rs/bollard/latest/bollard/auth/struct.DockerCredentials.html
    //
    // AWS ECR
    // https://docs.rs/aws-sdk-ecr/latest/aws_sdk_ecr/types/struct.AuthorizationData.html
    //

    let region_provider =
        RegionProviderChain::first_try(Some("us-east-1").map(Region::new))
            .or_default_provider()
            .or_else(Region::new("us-east-1"));

    let shared_config =
        aws_config::from_env().region(region_provider).load().await;
    let client = aws_sdk_ecr::Client::new(&shared_config);
    let token = client.get_authorization_token().send().await.unwrap();
    let authorization =
        token.authorization_data()[0].authorization_token().unwrap();
    let data = BASE64_STANDARD.decode(authorization.as_bytes()).unwrap();
    let parts = String::from_utf8(data).unwrap();
    let parts: Vec<&str> = parts.split(':').collect();
    // dbg!(&parts);
    // Example in go for split AuthorizationData
    // https://github.com/chialab/aws-ecr-get-login-password/blob/main/main.go
    (parts[0].to_string(), parts[1].to_string())
}

fn get_port_from_dockerfile(dockerfile: &str) -> Option<u16> {
    let dockerfile = Dockerfile::parse(dockerfile).unwrap();
    let mut port: u16 = 0;

    for stage in dockerfile.iter_stages() {
        println!(
            "stage #{} (parent: {:?}, root: {:?})",
            stage.index, stage.parent, stage.root
        );

        for ins in stage.instructions {
            match ins {
                dockerfile_parser::Instruction::Misc(misc) => {
                    if misc.instruction.content.as_str() == "EXPOSE" {
                        match misc.arguments.components.get(0).unwrap() {
                            dockerfile_parser::BreakableStringComponent::String(c)
                                => {
                                    port = c.content.trim().parse().unwrap();
                                    break;
                                }
                            _ => {},
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if port == 0 {
        None
    } else {
        Some(port)
    }
}

fn compress(dockerfile: &str) -> Vec<u8> {
    let mut header = tar::Header::new_gnu();
    header.set_path("Dockerfile").unwrap();
    header.set_size(dockerfile.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    let mut tar = tar::Builder::new(Vec::new());
    tar.append(&header, dockerfile.as_bytes()).unwrap();

    let uncompressed = tar.into_inner().unwrap();
    let mut c = flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    );
    c.write_all(&uncompressed).unwrap();
    c.finish().unwrap()
}

fn build_options(id: &str) -> BuildImageOptions<&str> {
    BuildImageOptions {
        t: id,
        dockerfile: "Dockerfile",
        version: BuilderVersion::BuilderBuildKit,
        pull: true,
        session: Some(String::from(id)),
        ..Default::default()
    }
}

async fn docker_connect() -> Docker {
    Docker::connect_with_socket_defaults().unwrap()
}

async fn build_image(docker: &Docker, id: &str, dockerfile_content: &str) {
    let compressed = compress(dockerfile_content);
    let build_image_options = build_options(id);

    let mut image_build_stream =
        docker.build_image(build_image_options, None, Some(compressed.into()));

    while let Some(Ok(bollard::models::BuildInfo {
        aux: Some(BuildInfoAux::BuildKit(inner)),
        ..
    })) = image_build_stream.next().await
    {
        println!("Response: {:?}", inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn aws_ecr_credential() {
        let _credential = get_credential().await;
        assert!(true);
    }

    // #[tokio::test]
    // async fn docker_build_image() {
    //     let client = docker_connect().await;
    //     let dockerfile = String::from(
    //         "FROM alpine as builder1
    // RUN touch bollard.txt
    // FROM alpine as builder2
    // RUN --mount=type=bind,from=builder1,target=mnt cp mnt/bollard.txt buildkit-bollard.txt
    // ENTRYPOINT ls buildkit-bollard.txt
    // ",
    //     );

    //     build_image(&client, "myimage", &dockerfile).await;

    //     assert!(true);
    // }

    // #[test]
    // fn get_port_dockerfile() {
    //     let dockerfile = String::from(
    //         "FROM alpine as builder1
    // RUN touch bollard.txt
    // FROM alpine as builder2
    // RUN --mount=type=bind,from=builder1,target=mnt cp mnt/bollard.txt buildkit-bollard.txt
    // EXPOSE 3000
    // ENTRYPOINT ls buildkit-bollard.txt
    //         "
    //     );
    //     let port = get_port_from_dockerfile(&dockerfile);
    //     assert!(port.is_some());
    //     assert_eq!(3000 as u16, port.unwrap());
    // }
}
