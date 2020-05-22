/* Expand a post's file when it is clicked. */
function onClickPostImage(ev) {
    ev.preventDefault();

    if (ev.target.dataset.isImage) {
        expandImage(ev.target);
    } else if (ev.target.dataset.isVideo) {
        expandVideo(ev.target);
    }
}

/* Expand a post's image. */
function expandImage(target) {
    if (target.dataset.expanded) {
        delete target.dataset.expanded;
        target.src = target.dataset.thumbUri;
    } else if (target.dataset.expanding) {
        delete target.dataset.expanding;
        target.src = target.dataset.thumbUri;
    } else {
        target.dataset.expanding = "expanding";
        target.src = target.dataset.uri;
    }
}

/* Expand a post's video. */
function expandVideo(target) {
    var close = document.createElement('a');
    close.href = "#";
    close.classList.add("video-close");
    close.textContent = "[Close]";

    close.addEventListener('click', function(ev) {
        ev.preventDefault();

        closeVideo(ev.target)
    });

    var video = document.createElement('video');
    video.controls = "controls";
    video.autoplay = "autoplay";
    video.loop = "loop";
    video.src = target.dataset.uri;
    video.poster = target.dataset.thumbUri;

    video.dataset.uri = target.dataset.uri;
    video.dataset.thumbUri = target.dataset.thumbUri;
    video.dataset.isVideo = "is-video";
    video.dataset.expanded = "expanded";

    target.parentNode.prepend(close);
    target.parentNode.replaceChild(video, target);
}

/* Close a post's video. */
function closeVideo(target) {
    var video = target.parentElement.querySelector('video');

    var image = document.createElement('img');
    image.src = video.dataset.thumbUri;

    image.dataset.uri = video.dataset.uri;
    image.dataset.thumbUri = video.dataset.thumbUri;
    image.dataset.isVideo = "is-video";
    image.addEventListener('click', onClickPostImage);

    target.parentNode.replaceChild(image, video);

    target.parentNode.removeChild(target);
}

/* Update the image's attributes once the image is done loading. */
function onLoadPostImage(ev) {
    if (ev.target.dataset.expanding) {
        delete ev.target.dataset.expanding;
        ev.target.dataset.expanded = "expanded";
    }
}

/* Display a preview of a post when the cursor hovers over a post reference. */
function onMouseEnterPostRef(ev) {
    var id = ev.target.textContent.replace(">>", "");
    var post = document.getElementById(id);

    var targetRect = ev.target.getBoundingClientRect();

    if (post === null) {
        var url = window.location.href.replace(/#.*/gi, "");
        url += "/preview/" + id;

        fetch(url)
            .then((response) => {
                return response.text();
            })
            .then((content) => {
                var template = document.createElement('template');
                template.innerHTML = content;

                addPostPreview(template.content.firstChild, targetRect);
            });
    } else {
        let postRect = post.getBoundingClientRect();

        if (postRect.top >= 0 && postRect.bottom <= window.innerHeight) {
            post.classList.add('highlighted');
        } else {
            addPostPreview(post.cloneNode(true), targetRect);
        }
    }
}

/* Insert a post preview into the DOM. */
function addPostPreview(postPreview, targetRect) {
    postPreview.classList.add('post-fixed');

    document.body.appendChild(postPreview);

    var previewRect = postPreview.getBoundingClientRect();

    var maxPreviewTop = window.innerHeight - previewRect.height - 40;

    var previewTop = targetRect.bottom - (previewRect.height / 2);
    previewTop = Math.max(previewTop, 0);
    previewTop = Math.min(previewTop, maxPreviewTop);

    var previewMaxWidth = window.innerWidth - targetRect.right - 40;

    postPreview.style.top = previewTop + "px";
    postPreview.style.left = (targetRect.right + 10) + "px";
    postPreview.style.maxWidth = previewMaxWidth + "px";
}

/* Remove a post preview when the cursor stops hovering over the post
 * reference. */
function onMouseLeavePostRef(ev) {
    document.querySelectorAll('.post-fixed').forEach((elem) => {
        elem.remove();
    });

    document.querySelectorAll('.highlighted').forEach((elem) => {
        elem.classList.remove('highlighted');
    });
}

document.addEventListener('DOMContentLoaded', () => {
    document.querySelectorAll('.post-image img').forEach((elem) => {
        elem.addEventListener('click', onClickPostImage);
        elem.addEventListener('load', onLoadPostImage);
    });

    document.querySelectorAll('.post .post-ref').forEach((elem) => {
        elem.addEventListener('mouseenter', onMouseEnterPostRef);
    });

    document.querySelectorAll('.post .post-ref').forEach((elem) => {
        elem.addEventListener('mouseleave', onMouseLeavePostRef);
    });
})
